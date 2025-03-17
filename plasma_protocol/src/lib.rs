// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![feature(const_option)]
#![feature(lazy_cell)]
#![feature(variant_count)]
#![feature(let_chains)]

mod claims;
mod client;
mod ids;
mod metrics;
mod names;
#[cfg(feature = "plasma")]
mod plasma;
mod quest;
mod serde_util;

pub use claims::*;
pub use client::*;
pub use ids::*;
pub use metrics::*;
pub use names::*;
#[cfg(feature = "plasma")]
pub use plasma::*;
pub use quest::*;
pub use serde_util::*;

// Re-export bitcode. (formerly core_protocol::prelude::bitcode)
pub use bitcode;

// Re-export some of the symbols from cub.
pub use cub::{
    impl_wrapper_display, impl_wrapper_from_str, impl_wrapper_str, is_default, serde_str,
    FromStrVisitor, NonZeroUnixMillis, UnixTime,
};

// Re-export rustrict. (formerly core_protocol::prelude::rustrict)
#[cfg(feature = "plasma")]
pub use rustrict;
