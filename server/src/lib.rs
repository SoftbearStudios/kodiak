// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![feature(extract_if)]
#![feature(new_uninit)]
#![feature(get_mut_unchecked)]
#![feature(async_closure)]
#![feature(hash_extract_if)]
#![feature(binary_heap_into_iter_sorted)]
#![feature(int_roundings)]
#![feature(associated_type_defaults)]
#![feature(is_sorted)]
#![feature(variant_count)]
#![feature(result_flattening)]
#![feature(let_chains)]
#![feature(lazy_cell)]
#![feature(map_many_mut)]
#![feature(inline_const)]
#![allow(incomplete_features)]
#![feature(array_chunks)]
#![feature(impl_trait_in_assoc_type)]
#![feature(round_char_boundary)]
#![feature(alloc_error_hook)]
#![feature(if_let_guard)]

mod actor;
mod entry_point;
mod files;
mod service;
#[macro_use]
mod util;
mod cli;
mod net;
mod observer;
mod rate_limiter;
mod router;
mod shutdown;
mod socket;
mod state;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// Export `pub` symbols below. Remaining symbols are effectively `pub(crate)`.
pub use entry_point::entry_point;
pub use service::{
    random_bot_name, random_bot_team_name, random_emoji_bot_name, ArenaContext, ArenaService, Bot,
    BotAction, BotOptions, Player, RedirectedPlayer, Score, ShardPerRealm, ShardPerTier,
};
pub use util::{base64_decode, base64_encode, diff_large_n, diff_small_n};

// Re-export kodiak_common.
pub use kodiak_common::{self, *};

// Re-export server tools.
pub mod prelude {
    pub use minicdn;
}
pub use minicdn;

// Re-export commonly-used third party crates.
pub use {base64, bytes, log};
