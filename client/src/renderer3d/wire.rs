// SPDX-FileCopyrightText: 2024 Softbear, Inc.

use crate::renderer::{
    derive_vertex, include_shader, DefaultRender, Layer, LineBuffer, RenderLayer, Renderer, Shader,
};
use crate::renderer3d::Camera3d;
use glam::Vec3;

/// A line between points.
pub struct Wire {
    /// The start point.
    pub a: Vec3,
    /// The end point.
    pub b: Vec3,
    /// The color of the wire.
    pub color: Vec3,
}

derive_vertex!(
    struct WireVertex {
        pos: Vec3,
        color: Vec3,
    }
);

/// Renders lines between points.
pub struct WireLayer {
    /// The wires to render, reset every frame.
    pub wires: Vec<Wire>,
    buffer: LineBuffer<WireVertex>,
    shader: Shader,
}

impl DefaultRender for WireLayer {
    fn new(renderer: &Renderer) -> Self {
        Self {
            wires: vec![],
            buffer: DefaultRender::new(renderer),
            shader: include_shader!(renderer, "wire"),
        }
    }
}

impl Layer for WireLayer {}

impl RenderLayer<&Camera3d> for WireLayer {
    fn render(&mut self, renderer: &Renderer, params: &Camera3d) {
        if let Some(shader) = self.shader.bind(renderer) {
            params.prepare_without_camera_pos(&shader);

            let vertices: Vec<_> = std::mem::take(&mut self.wires)
                .into_iter()
                .flat_map(|wire| {
                    [
                        WireVertex {
                            pos: wire.a,
                            color: wire.color,
                        },
                        WireVertex {
                            pos: wire.b,
                            color: wire.color,
                        },
                    ]
                })
                .collect();
            if vertices.is_empty() {
                return;
            }

            self.buffer.buffer(renderer, &vertices, &[]);
            self.buffer.bind(renderer).draw();
        }
    }
}
