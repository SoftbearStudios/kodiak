// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![warn(missing_docs)]

//! # Renderer3D
//!
//! [`renderer3d`][`crate`] is an add-on to [`renderer`] that provides a [`Camera3d`], and in the future, some
//! 3D specific [`Layer`][`renderer::Layer`]s.

extern crate core;

mod aabb;
mod camera_3d;
mod crosshair;
mod free_camera;
#[cfg(feature = "renderer3d_model")]
mod model;
#[cfg(feature = "renderer3d_shadow")]
mod shadow;
mod shadow_volume;
mod skybox;
#[cfg(feature = "renderer3d_model")]
mod svg;
mod text;
mod wire;

// Re-export to provide a simpler api.
pub use aabb::*;
pub use camera_3d::*;
pub use crosshair::*;
pub use free_camera::*;
#[cfg(feature = "renderer3d_model")]
pub use model::*;
#[cfg(feature = "renderer3d_shadow")]
pub use shadow::*;
pub use shadow_volume::*;
pub use skybox::*;
pub use text::*;
pub use wire::*;
