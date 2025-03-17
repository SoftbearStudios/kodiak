// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::LockstepWorld;
use crate::bitcode::{self, *};
use crate::{ArenaMap, PlayerId};
use std::collections::BTreeMap;
use std::fmt::{self, Debug, Formatter};

#[derive(Clone, Encode, Decode)]
pub struct LockstepTick<W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    pub checksum: Option<u32>,
    #[cfg(feature = "desync")]
    pub complete: Option<super::Lockstep<W>>,
    pub overwrites: BTreeMap<PlayerId, Option<W::Player>>,
    /// An input for all `players`.
    pub inputs: ArenaMap<PlayerId, W::Input>,
    pub inner: W::Tick,
}

impl<W: LockstepWorld> Default for LockstepTick<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn default() -> Self {
        Self {
            checksum: Default::default(),
            #[cfg(feature = "desync")]
            complete: Default::default(),
            overwrites: Default::default(),
            inputs: Default::default(),
            inner: Default::default(),
        }
    }
}

impl<W: LockstepWorld> Debug for LockstepTick<W>
where
    <W as LockstepWorld>::Player: Debug,
    <W as LockstepWorld>::Input: Debug,
    <W as LockstepWorld>::Tick: Debug,
    [(); W::LAG_COMPENSATION]:,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            checksum,
            #[cfg(feature = "desync")]
                complete: _,
            overwrites,
            inputs,
            inner,
        } = self;
        f.debug_struct("LockstepTick")
            .field("checksum", checksum)
            .field("overwrites", overwrites)
            .field("inputs", inputs)
            .field("inner", inner)
            .finish()
    }
}
