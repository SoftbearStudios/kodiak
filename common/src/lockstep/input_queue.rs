// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{LockstepInputId, LockstepInputWindow, LockstepWorld};
use arrayvec::ArrayVec;

pub struct LockstepInputQueue<W: LockstepWorld>
where
    [(); W::MAX_PREDICTION]:,
{
    inputs: ArrayVec<W::Input, { W::MAX_PREDICTION }>,
    pub end: LockstepInputId, // non inclusive (1 past inputs.last())
}

impl<W: LockstepWorld> Default for LockstepInputQueue<W>
where
    [(); W::MAX_PREDICTION]:,
{
    fn default() -> Self {
        Self {
            inputs: Default::default(),
            end: 1, // 0 is invalid CommandId. TODO make CommandId nonzero.
        }
    }
}

impl<W: LockstepWorld> LockstepInputQueue<W>
where
    [(); W::MAX_PREDICTION]:,
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
{
    /// Returns if the [`CommandQueue`] can't grow anymore. Avoids predicting too much to prevent
    /// client from freezing due to too many physics calculations.
    pub fn is_full(&self) -> bool {
        self.inputs.len() >= W::MAX_PREDICTION as usize
    }

    /// Pops the oldest element (the start of the [`CommandQueue`]).
    pub fn pop_front(&mut self) {
        // Doesn't change where self.end is.
        self.inputs.remove(0);
    }

    /// Pushes a new [`Input`] into the back of the [`CommandQueue`].
    ///
    /// Returns a [`CommandWindow`] to send to the server.
    ///
    /// **Panics**
    ///
    /// If `self.is_full()`.
    pub fn push_back(&mut self, input: W::Input, unreliable: bool) -> LockstepInputWindow<W> {
        debug_assert!(!self.is_full());
        let id = self.end;
        self.end = id.checked_add(1).unwrap();
        self.inputs.push(input);

        let sliding_window = if unreliable {
            W::INPUTS_PER_EFFICIENT_PACKET
        } else {
            1
        };
        LockstepInputWindow {
            sliding_window: self.inputs.as_slice()
                [self.inputs.len().saturating_sub(sliding_window)..]
                .iter()
                .copied()
                .collect(),
            last_input_id: id,
        }
    }

    /// Server has acknowledged all commands up to and including `id`.
    pub fn acknowledged(&mut self, last_applied_id: LockstepInputId) {
        let start = self.end - self.inputs.len() as LockstepInputId;
        if last_applied_id < start {
            return;
        }
        // Will panic if server acknowledges a command we haven't sent yet.
        self.inputs
            .drain(..=(last_applied_id - start) as usize)
            .count();
    }

    /// Iterates all the controls that are being predicted.
    pub fn iter(&self) -> impl Iterator<Item = W::Input> + '_ {
        self.inputs.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.inputs.len()
    }

    /// Returns the latency between the client and the server in ticks given the last [`CommandId`]
    /// received.
    pub fn latency(&self, received_id: LockstepInputId) -> u32 {
        (self.end - 1) - received_id
    }
}
