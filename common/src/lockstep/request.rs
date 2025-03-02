// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{LockstepInput, LockstepInputWindow, LockstepWorld};
use crate::bitcode::{self, *};
use std::fmt::{self, Debug, Formatter};

#[derive(Clone, Encode, Decode)]
pub struct LockstepRequest<W: LockstepWorld>
where
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
{
    pub inputs: LockstepInputWindow<W>,
}

impl<W: LockstepWorld> LockstepRequest<W>
where
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
{
    pub fn bot(input: W::Input) -> Self {
        Self {
            inputs: LockstepInputWindow::from(LockstepInput {
                inner: input,
                input_id: 0,
            }),
        }
    }
}

impl<W: LockstepWorld + Debug> Debug for LockstepRequest<W>
where
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
    W::Input: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { inputs } = self;
        f.debug_struct("LockstepRequest")
            .field("inputs", &inputs)
            .finish()
    }
}
