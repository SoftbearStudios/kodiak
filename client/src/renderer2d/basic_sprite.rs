// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Camera2d;
use crate::renderer::{
    derive_vertex, include_shader, DefaultRender, Layer, MeshBuilder, RenderLayer, Renderer,
    Shader, Texture, TextureFormat, TriangleBuffer,
};
use crate::sprite_sheet::UvSpriteSheet;
use kodiak_common::glam::{Mat3, Vec2};

derive_vertex!(
    struct SpriteVertex {
        pos: Vec2,
        uv: Vec2,
        alpha: f32,
    }
);

/// Draws sprites from a [`UvSpriteSheet`].
pub struct BasicSpriteLayer {
    atlas_color: Texture,
    buffer: TriangleBuffer<SpriteVertex>,
    mesh: MeshBuilder<SpriteVertex>,
    shader: Shader,
    sheet: UvSpriteSheet,
}

impl BasicSpriteLayer {
    /// Creates a new RGBA sprite layer with the contents of the sprite sheet in JSON
    /// and the path to the color texture.
    pub fn new(renderer: &Renderer, sheet_json: &str, texture_path: &str) -> Self {
        let sheet = serde_json::from_str(sheet_json).unwrap();

        let atlas_color = Texture::loader(renderer, texture_path, TextureFormat::COLOR_RGBA).load();

        let shader = include_shader!(renderer, "basic_sprite");

        Self {
            atlas_color,
            buffer: TriangleBuffer::new(renderer),
            mesh: MeshBuilder::new(),
            shader,
            sheet,
        }
    }

    /// Draws a sprite.
    pub fn draw(&mut self, sprite: &str, matrix: Mat3, alpha: f32) {
        let sprite = self.sheet.sprites.get(sprite).expect(sprite);

        let positions = [
            Vec2::new(-0.5, -0.5),
            Vec2::new(0.5, -0.5),
            Vec2::new(0.5, 0.5),
            Vec2::new(-0.5, 0.5),
        ];

        self.mesh.vertices.extend(
            IntoIterator::into_iter(positions)
                .zip(sprite.uvs.iter())
                .map(|(pos, &uv)| SpriteVertex {
                    pos: matrix.transform_point2(pos),
                    uv,
                    alpha,
                }),
        );
    }
}

impl Layer for BasicSpriteLayer {
    const ALPHA: bool = true;
}

impl RenderLayer<&Camera2d> for BasicSpriteLayer {
    fn render(&mut self, renderer: &Renderer, camera: &Camera2d) {
        if self.mesh.is_empty() {
            return;
        }

        if let Some(shader) = self.shader.bind(renderer) {
            camera.prepare(&shader);
            shader.uniform("uColor", &self.atlas_color);

            self.mesh.push_default_quads();
            self.buffer.buffer_mesh(renderer, &self.mesh);

            self.buffer.bind(renderer).draw();
        }

        // Always clear mesh even if shader wasn't bound.
        self.mesh.clear();
    }
}
