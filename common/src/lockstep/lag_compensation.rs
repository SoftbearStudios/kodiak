// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::LockstepWorld;
use crate::bitcode::{self, Decode, Encode};
use crate::HbHash;
use std::hash::Hash;

#[derive(Clone, Debug, Encode, Decode)]
pub struct LagCompensation<T, W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    lag_compensation: [Option<T>; W::LAG_COMPENSATION],
}

impl<T, W: LockstepWorld> Default for LagCompensation<T, W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn default() -> Self {
        Self {
            lag_compensation: std::array::from_fn(|_| None),
        }
    }
}

macro_rules! impl_hash {
    ($which:ident) => {
        impl<T: HbHash, W: LockstepWorld> $which for LagCompensation<T, W>
        where
            [(); W::LAG_COMPENSATION]:,
        {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.lag_compensation.hash(state);
            }
        }
    };
}

impl_hash!(Hash);
impl_hash!(HbHash);

impl<T, W: LockstepWorld> LagCompensation<T, W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn index(&self, tick_id: u32) -> usize {
        tick_id as usize % W::LAG_COMPENSATION
    }

    pub fn write(&mut self, tick_id: u32, value: T) {
        self.lag_compensation[self.index(tick_id)] = Some(value);
    }

    pub fn read(&self, tick_id: u32, latency: u8) -> Option<&T> {
        if latency > W::MAX_LATENCY {
            return None;
        }
        let past_tick_id = tick_id.wrapping_sub(latency as u32);
        self.lag_compensation[self.index(past_tick_id)].as_ref()
    }
}
