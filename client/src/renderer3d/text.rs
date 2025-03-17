// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Camera3d;
use crate::renderer::{
    include_shader, DefaultRender, GenTextLayer, Layer, Renderer, ShaderBinding, TextInstance,
    TextStyle,
};
use kodiak_common::glam::{vec3, Mat4, Quat, Vec3};

struct Text3d {
    center: Vec3,
    scale: f32,
}

impl TextInstance for Text3d {
    type Params<'a> = &'a Camera3d;

    fn prepare(&self, shader: &ShaderBinding, texture_aspect: f32, camera: &Camera3d) {
        let model_view = camera.view_matrix * Mat4::from_translation(self.center);
        let model = Mat4::from_scale_rotation_translation(
            vec3(self.scale * texture_aspect, self.scale, 0.0),
            Quat::from_mat4(&model_view).inverse(),
            self.center,
        );
        shader.uniform("uModelViewProjection", &(camera.vp_matrix * model));
    }
}

/// Draws single lines of text.
#[derive(Layer)]
#[alpha]
#[render(&Camera3d)]
pub struct TextLayer {
    inner: GenTextLayer<Text3d>,
}

impl DefaultRender for TextLayer {
    fn new(renderer: &Renderer) -> Self {
        Self {
            inner: GenTextLayer::new(renderer, include_shader!(renderer, "text")),
        }
    }
}

impl TextLayer {
    /// Draws `text` centered at `center` with a `scale` and a `color`.
    /// TODO `scale`'s units need to be more precisely defined.
    pub fn draw(&mut self, text: &str, center: Vec3, scale: f32, color: [u8; 4], style: TextStyle) {
        // Compensate for resizing text texture to 36 pixels to fit "😊". TODO find better solution.
        let scale = scale * (36.0 / 32.0);
        self.inner
            .draw(text, color, style, Text3d { center, scale });
    }
}
