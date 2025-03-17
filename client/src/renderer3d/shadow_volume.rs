// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::renderer::{
    include_shader, DefaultRender, Layer, RenderLayer, Renderer, Shader, TriangleBuffer,
};
use kodiak_common::glam::{vec2, Vec2, Vec4};
use std::ops::Deref;

/// Indicates that the shadow volumes should be rendered.
pub struct ShadowVolumeParams<P>(P);

impl<P> Deref for ShadowVolumeParams<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Renders `inner` and then renders the `inner`'s shadow volumes.
#[derive(Layer)]
#[stencil]
pub struct ShadowVolumeLayer<L> {
    /// The [`Layer`] passed to [`ShadowVolumeLayer::new`].
    #[layer]
    pub inner: L,
    buffer: TriangleBuffer<Vec2>,
    color: Vec4,
    shader: Shader,
}

impl<L> ShadowVolumeLayer<L> {
    /// Creates a new [`ShadowVolumeLayer`].
    pub fn new(renderer: &Renderer, inner: L, color: Vec4) -> Self {
        let mut buffer = TriangleBuffer::new(renderer);
        buffer.buffer(
            renderer,
            &[vec2(-1.0, 3.0), vec2(-1.0, -1.0), vec2(3.0, -1.0)],
            &[],
        );
        Self {
            inner,
            buffer,
            color,
            shader: include_shader!(renderer, "shadow_volume"),
        }
    }
}

impl<L, P: Copy> RenderLayer<P> for ShadowVolumeLayer<L>
where
    L: RenderLayer<P> + for<'a> RenderLayer<&'a ShadowVolumeParams<P>>,
{
    fn render(&mut self, renderer: &Renderer, params: P) {
        self.inner.render(renderer, params);

        renderer.set_depth_mask(false);
        renderer.shadow_volume_stencil(0);

        renderer.set_color_mask(false);
        // Invert depth to allow camera to be in shadow volume.
        // See depth fail: https://en.wikipedia.org/wiki/Shadow_volume
        renderer.invert_depth(true);
        self.inner.render(renderer, &ShadowVolumeParams(params));
        renderer.invert_depth(false);
        renderer.set_color_mask(true);

        renderer.shadow_volume_stencil(1);

        if let Some(shader) = self.shader.bind(renderer) {
            shader.uniform("uColor", self.color);
            self.buffer.bind(renderer).draw();
        }

        renderer.shadow_volume_stencil(2);
        renderer.set_depth_mask(true);
    }
}
