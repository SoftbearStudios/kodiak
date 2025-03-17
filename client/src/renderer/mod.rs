// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![warn(missing_docs)]
//! # Renderer
//!
//! [`renderer`][`crate`] is an abstraction over
//! [WebGL](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.WebGlRenderingContext.html)/
//! [WebGL2](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.WebGl2RenderingContext.html)
//! that can be used in 2D and 3D applications.

// Gl primitives should not escape this crate.
#[macro_use]
mod gl;

#[cfg(feature = "renderer_query")]
mod query;
#[cfg(feature = "renderer_srgb")]
mod srgb_layer;

mod antialiasing;
mod attribs;
mod buffer;
mod deque;
mod framebuffer;
mod index;
mod instance;
mod renderer;
mod rgb;
mod shader;
mod text;
mod texture;
mod toggle;
mod vertex;

// Required to be public so derive Vertex works.
#[doc(hidden)]
pub use attribs::*;

// Re-export to provide a simpler api.
#[cfg(feature = "renderer_query")]
pub use query::*;

pub use self::antialiasing::*;
pub use self::buffer::*;
pub use self::deque::*;
pub use self::framebuffer::*;
pub use self::index::*;
pub use self::instance::*;
pub use self::renderer::*;
pub use self::rgb::*;
pub use self::shader::*;
pub use self::text::*;
pub use self::texture::*;
pub use self::toggle::*;
pub use self::vertex::*;

/// Include a [`Shader`] at `shaders/$name.vert` and `shaders/$name.frag`.
pub macro include_shader {
    ($renderer:expr, $name:literal) => {
        include_shader!($renderer, $name, $name)
    },
    ($renderer:expr, $vertex:literal, $fragment:literal) => {
        $renderer.include_shader(
            include_str!(concat!("shaders/", $vertex, ".vert")),
            include_str!(concat!("shaders/", $fragment, ".frag")),
        )
    },
}
