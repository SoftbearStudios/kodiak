// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{Lockstep, LockstepInputId, LockstepTick, LockstepWorld};
use crate::bitcode::{self, *};
use crate::PlayerId;
use std::fmt::{self, Debug, Formatter};

#[derive(Clone, Encode, Decode)]
pub struct LockstepUpdate<W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    pub initialization: Option<(PlayerId, Lockstep<W>)>,
    pub last_applied_input_id: LockstepInputId,
    pub last_received_input_id: LockstepInputId,
    pub tick: LockstepTick<W>,
    pub buffered_inputs: usize,
}

impl<W: LockstepWorld + Debug> Debug for LockstepUpdate<W>
where
    [(); W::LAG_COMPENSATION]:,
    W::Player: Debug,
    W::Input: Debug,
    W::Tick: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            initialization,
            last_applied_input_id,
            last_received_input_id,
            tick,
            buffered_inputs,
        } = self;
        f.debug_struct("LockstepUpdate")
            .field("initialization", &initialization)
            .field("last_applied_input_id", last_applied_input_id)
            .field("last_received_input_id", last_received_input_id)
            .field("tick", &tick)
            .field("buffered_inputs", &buffered_inputs)
            .finish()
    }
}
