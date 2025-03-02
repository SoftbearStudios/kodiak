// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use crate::{PlayerId, TeamId};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// A key that can be mapped to a monotonically increasing integer.
pub trait ArenaKey: Copy {
    fn from_index(i: usize) -> Self;
    fn to_index(self) -> usize;
}

impl ArenaKey for PlayerId {
    fn from_index(i: usize) -> Self {
        PlayerId(((i + 1) as u16).try_into().unwrap())
    }

    fn to_index(self) -> usize {
        self.0.get() as usize - 1
    }
}

impl ArenaKey for TeamId {
    fn from_index(i: usize) -> Self {
        TeamId(((i + 1) as u16).try_into().unwrap())
    }

    fn to_index(self) -> usize {
        self.0.get() as usize - 1
    }
}

/// Maps increasing integers to values.
/// TODO impl actor::storage::Map.
#[derive(Clone, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct ArenaMap<K, V> {
    /// Invariant: Never ends in `None`.
    slots: Vec<Option<V>>,
    len: usize,
    _spooky: PhantomData<K>,
}

impl<K: ArenaKey + Debug, V: Debug> Debug for ArenaMap<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

// Can't derive since it would bound K + V: Default.
impl<K, V> Default for ArenaMap<K, V> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            len: 0,
            _spooky: PhantomData,
        }
    }
}

impl<K: ArenaKey, V> ArenaMap<K, V> {
    /// Same as `Self::default()`.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of items in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the number of slots in the map.
    pub fn capacity(&self) -> usize {
        self.slots.capacity()
    }

    /// Returns true iff the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Test if the key exists in the map.
    pub fn contains(&self, key: K) -> bool {
        self.get(key).is_some()
    }

    /// Gets a value.
    pub fn get(&self, key: K) -> Option<&V> {
        self.slots.get(key.to_index()).and_then(|p| p.as_ref())
    }

    /// Gets a value mutably.
    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.slots.get_mut(key.to_index()).and_then(|p| p.as_mut())
    }

    /// Gets two values mutably. Returns `None` if `a == b` or either is out of bounds.
    pub fn get_two_mut(&mut self, a: K, b: K) -> Option<(&mut V, &mut V)> {
        self.slots
            .get_many_mut([a.to_index(), b.to_index()])
            .ok()
            .and_then(|[a, b]| a.as_mut().zip(b.as_mut()))
    }

    /// Entry API like HashMap::entry.
    pub fn entry(&mut self, key: K) -> ArenaEntry<'_, K, V> {
        if self.contains(key) {
            ArenaEntry::Occupied(OccupiedEntry { map: self, key })
        } else {
            ArenaEntry::Vacant(VacantEntry { map: self, key })
        }
    }

    /// Inserts a `key` `value` pair, returning the old value.
    ///
    /// No longer panics if already contains key.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let i = key.to_index();
        if i >= self.slots.len() {
            self.slots.resize_with(i + 1, || None)
        }

        let slot = &mut self.slots[i];
        let old = std::mem::replace(slot, Some(value));
        if old.is_none() {
            self.len += 1;
        }
        old
    }

    /// Removes a `key` from the map.
    ///
    /// No longer panics if map doesn't contain key.
    pub fn remove(&mut self, key: K) -> Option<V> {
        let Some(slot @ Some(_)) = self.slots.get_mut(key.to_index()) else {
            return None;
        };
        let old = std::mem::take(slot);
        if old.is_some() {
            self.len -= 1;
        }
        self.shrink();
        old
    }

    /// Like `HashMap`` retain.
    pub fn retain<F: FnMut(K, &mut V) -> bool>(&mut self, mut f: F) {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(v) = slot
                && !f(K::from_index(i), v)
            {
                *slot = None;
                self.len -= 1;
            }
        }
        self.shrink();
    }

    /// Shrink map to speed up hash/iterate/serialize.
    fn shrink(&mut self) {
        while let Some(None) = self.slots.last() {
            self.slots.pop();
        }
    }

    /// Iterates key value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|v| (K::from_index(i), v)))
    }

    /// Iterates keys and values mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(i, v)| v.as_mut().map(|v| (K::from_index(i), v)))
    }

    /// Iterates only keys.
    pub fn keys(&self) -> impl Iterator<Item = K> + '_ {
        self.iter().map(|(k, _)| k)
    }

    /// Iterates only values.
    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.slots.iter().flatten()
    }

    /// Iterates only values mutably.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> + '_ {
        self.slots.iter_mut().flatten()
    }

    /// Iterates possible keys i.e. if it contains `[1, 3]` then it iterates `[0, 1, 2, 3]`.
    /// This has the advantage over [`Self::keys`] that the iterator doesn't borrow the map so
    /// mutation can take place while iterating.
    pub fn possible_keys(&self) -> impl Iterator<Item = K> + Clone {
        (0..self.slots.len()).map(|i| K::from_index(i))
    }

    /// Returns all possible pairs of keys, including pairs like (k1, k1).
    pub fn possible_key_pairs(&self) -> impl Iterator<Item = (K, K)> + Clone {
        let possible_keys = self.possible_keys();
        possible_keys
            .clone()
            .flat_map(move |k1| possible_keys.clone().map(move |k2| (k1, k2)))
    }

    /// Gets mutable access to the underlying data structure. Useful for using slice methods such as
    /// `split_at_mut`.
    pub fn raw_slots(&mut self) -> &'_ mut [Option<V>] {
        &mut self.slots
    }
}

impl<K: ArenaKey, V> Index<K> for ArenaMap<K, V> {
    type Output = V;

    fn index(&self, k: K) -> &Self::Output {
        self.get(k).unwrap()
    }
}

impl<K: ArenaKey, V> IndexMut<K> for ArenaMap<K, V> {
    fn index_mut(&mut self, k: K) -> &mut Self::Output {
        self.get_mut(k).unwrap()
    }
}

pub enum ArenaEntry<'a, K, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: ArenaKey, V> ArenaEntry<'a, K, V> {
    pub fn or_insert_with<F: FnOnce() -> V>(self, f: F) -> &'a mut V {
        match self {
            Self::Occupied(occupied) => occupied.into_mut(),
            Self::Vacant(vacant) => vacant.insert(f()),
        }
    }
}

pub struct OccupiedEntry<'a, K, V> {
    map: &'a mut ArenaMap<K, V>,
    key: K,
}

impl<'a, K: ArenaKey, V> OccupiedEntry<'a, K, V> {
    #[allow(unused)]
    pub fn get(&self) -> &V {
        &self.map[self.key]
    }

    pub fn get_mut(&mut self) -> &mut V {
        &mut self.map[self.key]
    }

    pub fn into_mut(self) -> &'a mut V {
        &mut self.map[self.key]
    }
}

pub struct VacantEntry<'a, K, V> {
    map: &'a mut ArenaMap<K, V>,
    key: K,
}

impl<'a, K: ArenaKey, V> VacantEntry<'a, K, V> {
    pub fn insert(self, v: V) -> &'a mut V {
        self.map.insert(self.key, v);
        &mut self.map[self.key]
    }
}

impl<K: ArenaKey, V> IntoIterator for ArenaMap<K, V> {
    type Item = (K, V);

    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.slots
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (K::from_index(i), v)))
    }
}
