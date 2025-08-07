// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![feature(extract_if)]
#![feature(hash_extract_if)]
#![feature(lazy_cell)]
#![feature(must_not_suspend)]
#![feature(associated_type_defaults)]
#![feature(variant_count)]
// Renderer
#![feature(cell_update)]
#![feature(decl_macro)]
#![feature(hash_raw_entry)]
#![feature(int_roundings)]
#![feature(slice_as_chunks)]
// Yew `tr`
#![feature(array_try_map)]
#![feature(let_chains)]
#![feature(array_chunks)]
#![feature(pattern)]
#![feature(stmt_expr_attributes)]
#![feature(iter_intersperse)]
#![feature(option_get_or_insert_default)]
#![feature(round_char_boundary)]
#![feature(num_midpoint)]

extern crate core;

mod broker;
mod browser;
mod fps;
mod game_client;
mod io;
mod js_hooks;
mod net;
#[cfg(any(feature = "renderer", feature = "renderer2d", feature = "renderer3d"))]
pub mod renderer;
#[cfg(feature = "renderer2d")]
pub mod renderer2d;
#[cfg(feature = "renderer3d")]
pub mod renderer3d;
mod sprite_sheet;
mod translation;
mod yew_ui;

// Export `pub` symbols below. Remaining symbols are effectively `pub(crate)`.
pub use self::broker::*;
pub use self::browser::*;
pub use self::fps::*;
pub use self::game_client::GameClient;
pub use self::io::*;
pub use self::js_hooks::*;
pub use self::net::{deep_connect, js_fetch, js_response_text};
pub use self::translation::*;
pub use self::yew_ui::*;
pub use sprite_sheet::*;

// Export symbols used by settings macros.
pub mod settings_prerequisites {
    pub use super::translation::Translator;
    pub use super::LocalSettings;
    pub use kodiak_common::translate;
}

// Re-export kodiak_common.
pub use kodiak_common::{self, *};

// Re-export markdown symbols from cub.
pub use cub::{markdown, MarkdownOptions};

// Re-export commonly-used third party crates.
// `yew` and `stylist` are ommitted since their macros require them to be direct dependencies.
pub use {js_sys, yew_router};
