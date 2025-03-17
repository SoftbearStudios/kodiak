// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{LockstepInput, LockstepInputId, LockstepWorld};
use crate::bitcode::{self, *};
use arrayvec::ArrayVec;

/// Like [`Command`] but contains multiple controls to mitigate effects of packet loss.
/// Since the sliding window is in order, only 1 [`InputId`] is required.
#[derive(Debug, Clone, Encode, Decode)]
pub struct LockstepInputWindow<W: LockstepWorld>
where
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
{
    // May contain multiple controls to reduce impact of packet loss.
    // The last control has `id`. The previous one `id - 1` and so on.
    pub sliding_window: ArrayVec<W::Input, { W::INPUTS_PER_EFFICIENT_PACKET }>,
    pub last_input_id: LockstepInputId,
}

impl<W: LockstepWorld> LockstepInputWindow<W>
where
    [(); W::MAX_PREDICTION]:,
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
{
    /// Converts [`CommandWindow`] into an iterator of [`Command`]s from oldest to newest.
    pub fn into_input_iter(self) -> impl Iterator<Item = LockstepInput<W::Input>> {
        let len = self.sliding_window.len();
        self.sliding_window
            .into_iter()
            .enumerate()
            .map(move |(i, input)| {
                // Put here to avoid overflow if client sends nothing.
                let last_index = len - 1;
                let reverse_index = (last_index - i) as LockstepInputId;
                LockstepInput {
                    inner: input,
                    input_id: self.last_input_id.saturating_sub(reverse_index), // Shouldn't saturate unless client is buggy or malicious.
                }
            })
    }
}

impl<W: LockstepWorld> From<LockstepInput<W::Input>> for LockstepInputWindow<W>
where
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
{
    fn from(v: LockstepInput<W::Input>) -> Self {
        let mut sliding_window = ArrayVec::new();
        sliding_window.push(v.inner);
        Self {
            sliding_window,
            last_input_id: v.input_id,
        }
    }
}
