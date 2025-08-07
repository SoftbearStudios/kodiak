// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![feature(array_windows)]
#![feature(associated_type_defaults)]
#![feature(extend_one)]
#![feature(fundamental)]
#![feature(get_many_mut)]
#![feature(impl_trait_in_assoc_type)]
#![feature(int_roundings)]
#![feature(lazy_cell)]
#![feature(is_sorted)]
#![feature(let_chains)]
#![feature(never_type)]
#![feature(option_get_or_insert_default)]
#![feature(test)]
#![feature(vec_push_within_capacity)]
#![feature(with_negative_coherence)]
#![feature(generic_const_exprs)]
#![allow(incomplete_features)]
#![feature(coerce_unsized)]
#![feature(unsize)]

// Actually required see https://doc.rust-lang.org/beta/unstable-book/library-features/test.html
#[cfg(test)]
extern crate core;
#[cfg(test)]
extern crate test;
// If HbHash is used within kodiak_common.
// extern crate self as kodiak_common;

pub mod actor_model;
mod alloc;
mod collection;
mod game_constants;
mod lockstep;
mod math;
mod protocol;
mod team;
mod time;

// Not all `actor_model` symbols are exported directly.
pub use actor_model::{
    Entities2d, Entity2d, EntityIndex2d, OutOfBounds, SectorArray2d, SectorId2d, SectorMap2d,
};
pub use alloc::*;
pub use collection::*;
pub use game_constants::*;
pub use lockstep::*;
pub use math::*;
pub use protocol::*;
#[allow(unused)]
pub use team::*;
pub use time::*;

// Export symbols used by translation macros.
pub mod translation_prerequisites {
    pub use super::alloc::RcPtrEq;
}

// Re-export procedural macros.
pub use kodiak_macros::*;

// Re-export plasma_protocol.
pub use plasma_protocol::*;

// Re-export commonly-used third party crates.
pub use {arrayvec, fastapprox, fxhash, glam, heapless, rand, serde};
