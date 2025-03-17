// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::camera_2d::Camera2d;
use crate::renderer::{
    include_shader, DefaultRender, GenTextLayer, Layer, Renderer, ShaderBinding, TextInstance,
    TextStyle,
};
use kodiak_common::glam::{vec2, Mat3, Vec2};

struct Text2d {
    center: Vec2,
    scale: f32,
}

impl TextInstance for Text2d {
    type Params<'a> = &'a Camera2d;

    fn prepare(&self, shader: &ShaderBinding, texture_aspect: f32, camera: &Camera2d) {
        let model = Mat3::from_scale_angle_translation(
            vec2(self.scale * texture_aspect, self.scale),
            0.0,
            self.center,
        );
        shader.uniform("uModelView", &(camera.view_matrix * model));
    }
}

/// Draws single lines of text.
#[derive(Layer)]
#[alpha]
#[render(&Camera2d)]
pub struct TextLayer {
    inner: GenTextLayer<Text2d>,
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
    pub fn draw(&mut self, text: &str, center: Vec2, scale: f32, color: [u8; 4], style: TextStyle) {
        // Compensate for resizing text texture to 36 pixels to fit "😊". TODO find better solution.
        let scale = scale * (36.0 / 32.0);
        self.inner
            .draw(text, color, style, Text2d { center, scale });
    }
}
