// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Camera3d;
use crate::renderer::{
    gray, include_shader, DefaultRender, Layer, RenderLayer, Renderer, Shader, TriangleBuffer,
};
use kodiak_common::glam::{vec2, Vec2};

const THICKNESS: f32 = 0.0066;
const RADIUS: f32 = 0.025;

/// A [`Layer`] that renders a crosshair in the center of the screen.
pub struct CrosshairLayer {
    buffer: TriangleBuffer<Vec2>,
    shader: Shader,
    /// Opacity of the crosshair.
    pub alpha: f32,
}

impl DefaultRender for CrosshairLayer {
    fn new(renderer: &Renderer) -> Self {
        let mut buffer = TriangleBuffer::new(renderer);

        let t = (THICKNESS / RADIUS) * 0.5;
        let vertices = [
            vec2(-t, -t),
            vec2(t, -t),
            vec2(-t, t),
            vec2(t, t),
            vec2(-1.0, -t),
            vec2(-1.0, t),
            vec2(-t, -1.0),
            vec2(t, -1.0),
            vec2(1.0, -t),
            vec2(1.0, t),
            vec2(-t, 1.0),
            vec2(t, 1.0),
        ];
        let indices = [
            0, 1, 2, 3, 2, 1, 4, 0, 2, 5, 4, 3, 4, 0, 2, 5, 4, 3, 6, 1, 0, 1, 6, 7, 8, 3, 1, 3, 8,
            9, 2, 3, 10, 3, 11, 10,
        ];
        buffer.buffer(renderer, &vertices, &indices);

        let shader = include_shader!(renderer, "crosshair");
        Self {
            buffer,
            shader,
            alpha: 1.0,
        }
    }
}

impl Layer for CrosshairLayer {
    const ALPHA: bool = true;
}

impl RenderLayer<&Camera3d> for CrosshairLayer {
    fn render(&mut self, renderer: &Renderer, _: &Camera3d) {
        if let Some(shader) = self.shader.bind(renderer) {
            let scale = Vec2::splat(RADIUS) * vec2(renderer.aspect_ratio().recip(), 1.0);
            let color = (gray(200) * self.alpha).extend(self.alpha);

            shader.uniform("uScale", scale);
            shader.uniform("uColor", color);

            self.buffer.bind(renderer).draw();
        }
    }
}
