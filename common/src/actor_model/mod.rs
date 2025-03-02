// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

// Hours wasted trying to make it generic: 20

mod context;
// apply!, apply_inputs!, define_actor_state!, define_events!, define_world!, singleton!, singleton_mut!
mod macros;
mod sector_2d;
mod singletons;
mod storage;
mod tests;

// Also: define_on!
pub use self::context::{Dst, Src};
pub use self::sector_2d::{
    Entities2d, Entity2d, EntityIndex2d, OutOfBounds, SectorArray2d, SectorId2d, SectorMap2d,
};
pub use self::singletons::{
    Accumulate, Actor, ActorId, ActorKnowledge, Checksum, IsActive, Message, SequentialInbox,
    Server, TrivialBounds, WorldTick,
};
pub use self::storage::{
    Efficient, Map, NonexistentMap, OrdIter, Set, SortedVecMap, Sparse, Wrapper,
};

// Re-export paste.
pub use paste::paste;
