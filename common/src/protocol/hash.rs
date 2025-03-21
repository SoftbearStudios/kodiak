// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use fxhash::FxHasher32;
use glam::{Affine2, Affine3A, Mat2, Mat3, Mat3A, Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
pub use kodiak_macros::HbHash;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

/// Honey Badger Hash (doesn't care about restrictions against hashing floats)
pub trait HbHash {
    fn hash<H: Hasher>(&self, state: &mut H);
    fn hash_to_u64(&self) -> u64 {
        let mut hasher = CompatHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl HbHash for f32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32(*self, state)
    }
}

macro_rules! hb_hash_f32s {
    ($($t:ty),*) => {
        $(
            impl HbHash for $t {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    hash_f32s(self, state);
                }
            }
        )*
    }
}

hb_hash_f32s!(Vec2, Vec3, Vec3A, Vec4, Mat2, Mat3, Mat4, Quat);

impl<T: HbHash, const N: usize> HbHash for [T; N] {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for item in self {
            item.hash(state);
        }
    }
}

impl<T: HbHash> HbHash for Option<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(value) = self {
            1u8.hash(state);
            value.hash(state);
        } else {
            0u8.hash(state);
        }
    }
}

macro_rules! hb_hash_struct {
    ($($t:ty { $($field:ident),+}),+) => {
        $(
            impl HbHash for $t {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    $(
                        self.$field.hash(state);
                    )+
                }
            }
        )+
    }
}
hb_hash_struct!(
    Affine2 {
        matrix2,
        translation
    },
    Mat3A {
        x_axis,
        y_axis,
        z_axis
    },
    Affine3A {
        matrix3,
        translation
    }
);

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[repr(transparent)]
pub struct Hashable<T>(pub T);

impl<T> Hashable<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Hashable<T> {
    fn from(t: T) -> Self {
        Self(t)
    }
}

impl<T> Deref for Hashable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Hashable<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Display> Display for Hashable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Hash for Hashable<f32> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32(self.0, state);
    }
}

impl Hash for Hashable<Vec2> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32s(self.0, state);
    }
}

macro_rules! impl_hash_set_or_map {
    () => {
        fn hash<H: Hasher>(&self, state: &mut H) {
            let mut hash = 0;
            for v in &self.0 {
                // We can't bound H: Default so we have to assume a CompatHasher.
                let mut h = CompatHasher::default();
                v.hash(&mut h);
                hash ^= h.finish();
            }
            state.write_usize(self.len());
            state.write_u64(hash);
        }
    };
}

impl<T: Hash, S> Hash for Hashable<HashSet<T, S>> {
    impl_hash_set_or_map!();
}

impl<K: Hash, V: Hash, S> Hash for Hashable<HashMap<K, V, S>> {
    impl_hash_set_or_map!();
}

pub fn hash_f32<H: Hasher>(f: f32, state: &mut H) {
    state.write_u32(if f == 0.0 || f.is_nan() {
        debug_assert!(!f.is_nan(), "hash_float(NaN)");
        0
    } else {
        f.to_bits()
    });
}

pub fn hash_f32_ref<H: Hasher>(f: &f32, state: &mut H) {
    hash_f32(*f, state);
}

pub fn hash_f32s<H: Hasher, const N: usize>(floats: impl AsRef<[f32; N]>, state: &mut H) {
    for float in floats.as_ref() {
        hash_f32(*float, state);
    }
}

/// A hasher that converts usize to u32 and all integers to little endian bytes for compatibility.
#[derive(Default)]
pub struct CompatHasher {
    inner: FxHasher32,
}

macro_rules! impl_write {
    ($t:ty, $f:ident) => {
        #[inline]
        fn $f(&mut self, i: $t) {
            self.write(&i.to_le_bytes())
        }
    };
}

impl Hasher for CompatHasher {
    impl_write!(u8, write_u8);

    impl_write!(u16, write_u16);

    impl_write!(u32, write_u32);

    impl_write!(u64, write_u64);

    impl_write!(u128, write_u128);

    fn finish(&self) -> u64 {
        self.inner.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.inner.write(bytes)
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u32(i as u32); // Shouldn't be using any more than 32 bits of usize.
    }
}
