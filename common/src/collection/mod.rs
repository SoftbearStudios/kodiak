// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod alloc;
mod arena_map;
mod mask;
mod tests;

pub use self::alloc::{arc_default_n, box_default_n};
pub use self::arena_map::{ArenaEntry, ArenaKey, ArenaMap, OccupiedEntry, VacantEntry};
pub use self::mask::{Mask2d, Rect};
