// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::LockstepWorld;
use crate::bitcode::{self, *};
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Encode, Decode)]
pub struct LockstepPlayer<W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    /// Game specific input queued elsewhere but cached here to render player.
    pub input: W::Input,
    /// Game specific player state as of the tick that `input` was created.
    pub inner: W::Player,
}

impl<W: LockstepWorld> Deref for LockstepPlayer<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    type Target = W::Player;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<W: LockstepWorld> DerefMut for LockstepPlayer<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<W: LockstepWorld> Debug for LockstepPlayer<W>
where
    [(); W::LAG_COMPENSATION]:,
    W::Player: Debug,
    W::Input: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { input, inner } = self;
        f.debug_struct("LockstepPlayer")
            .field("input", input)
            .field("inner", inner)
            .finish()
    }
}

impl<W: LockstepWorld> Hash for LockstepPlayer<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        let Self { input, inner } = self;
        input.hash(state);
        inner.hash(state);
    }
}
