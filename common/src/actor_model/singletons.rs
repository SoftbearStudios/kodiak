// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{Efficient, Map, OrdIter, SortedVecMap, Sparse, Wrapper};
use crate::bitcode::{self, *};
use crate::{ArenaMap, CompatHasher, PlayerId, TeamId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

/// An [`Actor`] identifier.
pub trait ActorId: Copy {
    /// A [`Map`] that:
    /// - supports efficient insertions
    /// - iterates its keys based on [`Ord`]
    /// E.g. a 2d array.
    type DenseMap<T>: Map<Self, T> + Efficient + OrdIter = Self::SparseMap<T>;

    /// A [`Map`] that:
    /// - iterates its keys based on [`Ord`]
    /// - allocates memory proportional to its len
    /// E.g. a [`SortedVecMap`].
    type Map<T>: Map<Self, T> + OrdIter + Sparse;

    /// A [`Map`] that:
    /// - supports efficient insertions
    /// - iterates its keys based on [`Ord`]
    /// - allocates memory proportional to its len
    /// E.g. a [`HashMap`][`std::collections::HashMap`].
    type SparseMap<T>: Map<Self, T> + Efficient + OrdIter + Sparse;
}

impl ActorId for PlayerId {
    type DenseMap<T> = ArenaMap<Self, T>;
    // TODO better sparse/dense map.
    type Map<T> = SortedVecMap<Self, T>;
    type SparseMap<T> = BTreeMap<Self, T>;
}

impl ActorId for TeamId {
    type DenseMap<T> = ArenaMap<Self, T>;
    // TODO better sparse/dense map.
    type Map<T> = SortedVecMap<Self, T>;
    type SparseMap<T> = BTreeMap<Self, T>;
}

// TODO don't require Serialize
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Server;

impl ActorId for Server {
    // 1 bit overhead but never used.
    type Map<T> = Wrapper<Self, T>;
    type SparseMap<T> = Option<(Self, T)>; // 0 bits overhead.
}

/// A discrete unit within the world. The server has all of them and each client has a subset.
pub trait Actor: Clone {
    type Id: ActorId;

    /// How many ticks the [`Actor`] is kept after it is no longer visible.
    const KEEPALIVE: u8 = 5;
}

/// An inbox that applies it's [`Message`]s in the same order as they arrive.
pub trait SequentialInbox {}

impl<T, D: std::ops::Deref<Target = [T]>> SequentialInbox for D {}

/// A mutation that can be sent to an [`Actor`].
pub trait Message: Clone {
    type Inbox: Clone + Default + Extend<Self> = Vec<Self>;
}

/// A type that can report if the client has the [`Actor`]s associated with an [`ActorId`].
pub trait IsActive<Id> {
    fn is_active(&self, id: Id) -> bool;

    fn is_inactive(&self, id: Id) -> bool {
        !self.is_active(id)
    }
}

/// Like `Apply` but order does not matter.
pub trait Accumulate<T> {
    fn accumulate(&mut self, t: T);
}

/// A type that provides a level of desync detection.
pub trait Checksum: PartialEq {
    fn diff(&self, server: &Self) -> String;

    /// Can skip accumulates if this returns false.
    fn is_some(&self) -> bool {
        true
    }
}

impl<T> Accumulate<T> for () {
    fn accumulate(&mut self, _: T) {
        // No-op
    }
}

impl Checksum for () {
    fn diff(&self, _: &Self) -> String {
        String::new()
    }

    fn is_some(&self) -> bool {
        false
    }
}

impl<T: Hash> Accumulate<T> for u32 {
    fn accumulate(&mut self, t: T) {
        let mut hasher = CompatHasher::default();
        t.hash(&mut hasher);
        *self ^= hasher.finish() as u32
    }
}

impl Checksum for u32 {
    fn diff(&self, server: &Self) -> String {
        format!("client: {self:?} server: {server:?}")
    }
}

// TODO HashMap/BTreeMap based checksums.

/// Implement on result of [`define_world`] to provide [`tick_client`][`Self::tick_client`].
pub trait WorldTick<C: ?Sized> {
    /// TODO maybe remove everything but tick_client from this trait.
    /// The part of the tick before inputs arrive. Put as much as possible here to reduce latency.
    fn tick_before_inputs(&mut self, context: &mut C);
    /// The part of the tick after inputs are applied. Useful for things which depend on inputs
    /// being applied, such as applying events created by inputs.
    fn tick_after_inputs(&mut self, context: &mut C) {
        let _ = context;
    }
    /// Tick code that gets run on client during update apply.
    fn tick_client(&mut self, context: &mut C);
}

/// A client's knowledge of a particular [`Actor`].
#[derive(Debug)]
pub struct ActorKnowledge {
    /// Starts at [`Self::NEW`], gets set to `keepalive + 1`, then counts down each tick.
    counter: u8,
}

impl Default for ActorKnowledge {
    fn default() -> Self {
        Self { counter: Self::NEW }
    }
}

impl ActorKnowledge {
    /// Sentinel value to indicate that the actor is new.
    const NEW: u8 = u8::MAX;

    /// Was added this tick.
    pub fn is_new(&self) -> bool {
        self.counter == Self::NEW
    }

    /// Can send/receive events. Not [`Self::is_new`] and not [`Self::is_expired`].
    pub fn is_active(&self) -> bool {
        !self.is_new() && !self.is_expired()
    }

    /// Will be removed this tick.
    pub fn is_expired(&self) -> bool {
        self.counter == 0
    }

    /// Called at the beginning up an update. Resets the keepalive. Returns true if it's the first
    /// refresh this tick (not a duplicate).
    pub fn refresh(&mut self, keepalive: u8) -> bool {
        // Start at keepalive + 1 so a keepalive of 0 is valid.
        let counter = keepalive + 1;
        debug_assert_ne!(counter, Self::NEW);

        // Refresh can be called on a new knowledge or multiple times on an existing knowledge if
        // there are duplicates in visibility.
        let is_first = self.counter != counter && !self.is_new();
        if is_first {
            self.counter = counter
        }
        is_first
    }

    /// Called at the beginning of an update.
    pub fn tick(&mut self, keepalive: u8) {
        // Clear sentinel value.
        if self.is_new() {
            // Start at keepalive + 1 so a keepalive of 0 is valid.
            let c = keepalive + 1;
            debug_assert_ne!(c, Self::NEW);
            self.counter = c;
        }

        debug_assert_ne!(self.counter, 0, "expired knowledge wasn't cleared");
        self.counter -= 1;
    }
}

/// Work around for avoiding #[feature(trivial_bounds)].
#[doc(hidden)]
pub struct TrivialBounds<'a, T>(std::marker::PhantomData<&'a ()>, pub T);
impl<T: Default> Default for TrivialBounds<'_, T> {
    fn default() -> Self {
        Self(Default::default(), Default::default())
    }
}

// TODO remove this or make it more general (any tuple).
// Invariant: contents are sorted.
/*
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Encode, Decode)]
pub struct Pair<Id>([Id; 2]);

impl<Id: ActorId + PartialOrd> Pair<Id> {
    pub fn one(a: Id) -> Self {
        Self([a, a])
    }

    pub fn one_or_two(a: Id, b: Id) -> Self {
        if a < b {
            Self([a, b])
        } else {
            Self([b, a])
        }
    }
}

impl<Id: ActorId + Ord> ActorId for Pair<Id> {
    type DenseMap<T> = NonexistentMap<Self, T>;
    // TODO SparseMap<SparseMap<T>> if neither is BTreeMap.
    type Map<T> = SortedVecMap<Self, T>;
    type SparseMap<T> = BTreeMap<Self, T>; // TODO Map<Map<T>> if neither is SortedVecMap.
}

impl<T, Id: ActorId> IsActive<Pair<Id>> for T
where
    T: IsActive<Id>,
{
    fn is_active(&self, id: Pair<Id>) -> bool {
        self.is_active(id.0[0]) && self.is_active(id.0[1])
    }
}
*/
