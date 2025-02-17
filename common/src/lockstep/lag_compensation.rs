// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::LockstepWorld;

pub struct LagCompensation<T, W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    pub lag_compensation: [Option<T>; W::LAG_COMPENSATION],
}
