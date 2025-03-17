// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![warn(missing_docs)]
//! # Renderer2D
//!
//! [`renderer2d`][`crate`] is an add-on to [`renderer`] that provides a [`Camera2d`] and some 2D specific
//! [`Layer`][`renderer::Layer`]s.

mod background;
mod basic_sprite;
mod camera_2d;
mod graphic;
mod particle;
mod text;

pub use self::background::*;
pub use self::basic_sprite::*;
pub use self::camera_2d::*;
pub use self::graphic::*;
pub use self::particle::*;
pub use self::text::*;
