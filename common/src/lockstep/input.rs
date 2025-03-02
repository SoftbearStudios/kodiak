// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::fmt::{self, Debug, Formatter};
pub type LockstepInputId = u32;

pub struct LockstepInput<I> {
    pub input_id: LockstepInputId,
    pub inner: I,
}

impl<I: Debug> Debug for LockstepInput<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { input_id, inner } = self;
        f.debug_struct("LockstepInput")
            .field("input_id", input_id)
            .field("inner", inner)
            .finish()
    }
}
