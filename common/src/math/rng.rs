// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::CompatHasher;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use std::hash::{Hash, Hasher};

pub struct HashRng {
    inner: StdRng,
}

impl HashRng {
    pub fn new<S: Hash>(seed: &S) -> Self {
        let mut hasher = CompatHasher::default();
        seed.hash(&mut hasher);
        Self {
            inner: StdRng::seed_from_u64(hasher.finish()),
        }
    }
}

impl RngCore for HashRng {
    fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.inner.try_fill_bytes(dest)
    }
}
