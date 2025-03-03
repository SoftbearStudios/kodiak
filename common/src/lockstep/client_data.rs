// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{LockstepInput, LockstepInputId, LockstepWorld};
use arrayvec::ArrayVec;
use std::fmt::{self, Debug, Formatter};

pub struct LockstepClientData<W: LockstepWorld>
where
    [(); W::BUFFERED_TICKS]:,
{
    pub initialized: bool,
    /// Last command server incorporated into a tick.
    pub last_applied_command_id: LockstepInputId, // Default is 0, which client never sends.
    /// For latency calculation.
    pub last_received_command_id: LockstepInputId,
    pub receive_buffer: ArrayVec<LockstepInput<W::Input>, { W::BUFFERED_TICKS }>,
}

impl<W: LockstepWorld> Default for LockstepClientData<W>
where
    [(); W::BUFFERED_TICKS]:,
{
    fn default() -> Self {
        Self {
            initialized: Default::default(),
            last_applied_command_id: Default::default(),
            last_received_command_id: Default::default(),
            receive_buffer: Default::default(),
        }
    }
}

impl<W: LockstepWorld> Debug for LockstepClientData<W>
where
    W::Input: Debug,
    [(); W::BUFFERED_TICKS]:,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            initialized,
            last_applied_command_id,
            last_received_command_id,
            receive_buffer,
        } = self;
        f.debug_struct("LockstepClientData")
            .field("initialized", initialized)
            .field("last_applied_command_id", last_applied_command_id)
            .field("last_received_command_id", last_received_command_id)
            .field("receive_buffer", receive_buffer)
            .finish()
    }
}
