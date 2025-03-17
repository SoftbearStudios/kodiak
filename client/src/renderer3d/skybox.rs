// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Camera3d;
use crate::renderer::{
    include_shader, DefaultRender, Layer, RenderLayer, Renderer, Shader, Texture, TextureType,
    TriangleBuffer,
};
use kodiak_common::glam::{vec2, Vec2};

/// A [`Layer`] that renders a skybox. Must be the first layer for now.
pub struct SkyboxLayer {
    buffer: TriangleBuffer<Vec2>,
    cube_map: Texture,
    shader: Shader,
}

impl SkyboxLayer {
    /// Creates a new [`SkyboxLayer`] from a `cube_map` ([`Texture::typ`] == [`TextureType::Cube`]).
    pub fn with_cube_map(renderer: &Renderer, cube_map: Texture) -> Self {
        assert_eq!(
            cube_map.typ(),
            TextureType::Cube,
            "texture must be a cube map"
        );

        // Create a buffer that has 1 triangle covering the whole screen.
        let mut buffer = TriangleBuffer::new(renderer);
        buffer.buffer(
            renderer,
            &[vec2(-1.0, 3.0), vec2(-1.0, -1.0), vec2(3.0, -1.0)],
            &[],
        );

        Self {
            buffer,
            cube_map,
            shader: include_shader!(renderer, "skybox"),
        }
    }
}

impl Layer for SkyboxLayer {}

impl RenderLayer<&Camera3d> for SkyboxLayer {
    fn render(&mut self, renderer: &Renderer, params: &Camera3d) {
        if let Some(shader) = self.shader.bind(renderer) {
            renderer.invert_depth_equal(false); // Skybox depth is 1.0 and cleared depth is 1.0 so <= is required.
            renderer.set_depth_mask(false);

            let mut view_matrix = params.view_matrix;
            let v = view_matrix.as_mut();
            v[12] = 0.0;
            v[13] = 0.0;
            v[14] = 0.0;
            let matrix = (params.projection_matrix * view_matrix).inverse();

            shader.uniform("uMatrix", &matrix);
            shader.uniform("uSampler", &self.cube_map);
            self.buffer.bind(renderer).draw();

            renderer.invert_depth(false); // Set back to default: LESS.
            renderer.set_depth_mask(true);
        }
    }
}
