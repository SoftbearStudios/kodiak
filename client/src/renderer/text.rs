// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{
    DefaultRender, Layer, RenderLayer, Renderer, Shader, ShaderBinding, Texture, TriangleBuffer,
};
use kodiak_common::glam::{vec2, Vec2};
use std::collections::HashMap;
use std::hash::BuildHasher;

/// An instance of text such as a 2d or 3d name.
#[doc(hidden)]
pub trait TextInstance {
    /// Additional parameters such as the camera.
    type Params<'a>: Copy;
    /// Set uniforms required to render this instance.
    fn prepare(&self, shader: &ShaderBinding, texture_aspect: f32, params: Self::Params<'_>);
}

/// Font style.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Hash)]
pub enum TextStyle {
    /// Neither italic nor bold.
    #[default]
    Normal,
    /// Italic font.
    Italic,
    /// Bold font.
    Bold,
}

impl TextStyle {
    /// Constructs `Self::Italic` if `true`, otherwise `Self::Normal`.
    pub fn italic_if(italic: bool) -> Self {
        if italic {
            Self::Italic
        } else {
            Self::Normal
        }
    }
}

struct Buffers<M> {
    counter: u8,
    instances: Vec<M>,
    texture: Option<Texture>,
}

// Avoid bounding M: Default.
impl<M> Default for Buffers<M> {
    fn default() -> Self {
        Self {
            counter: Default::default(),
            instances: Default::default(),
            texture: Default::default(),
        }
    }
}

/// Generic text layer that can be adapted to draw 2d or 3d text.
#[doc(hidden)]
pub struct GenTextLayer<M> {
    /// Too expensive to create text textures every frame, so cache them.
    /// Index on text and color to allow CanvasRenderingContext to apply correct coloring to emojis.
    /// Uses 8 bit rbga color (compatible with JS).
    /// TODO could use additive blend mode to prevent unstable ordering if it matters.
    buffers: HashMap<(String, [u8; 4], TextStyle), Buffers<M>>,
    geometry: TriangleBuffer<Vec2>,
    shader: Shader,
}

impl<M: TextInstance> GenTextLayer<M> {
    /// Creates a new [`GenTextLayer`] given a `renderer` and a `shader` that takes `in vec2 position`
    /// and any uniforms set in [`TextInstance::prepare`].
    pub fn new(renderer: &Renderer, shader: Shader) -> Self {
        let mut geometry = TriangleBuffer::new(renderer);
        geometry.buffer(
            renderer,
            &[
                vec2(-0.5, -0.5),
                vec2(0.5, -0.5),
                vec2(0.5, 0.5),
                vec2(-0.5, 0.5),
            ],
            &[0, 1, 2, 2, 3, 0],
        );
        Self {
            buffers: Default::default(),
            geometry,
            shader,
        }
    }

    /// Draws `text` centered at `center` with a `scale` and a `color`. TODO `scale`'s units need
    /// to be more precisely defined.
    /// # Important
    /// Change the hash_one and from_hash arguments to match draw arguments
    /// or the CPU usage will rise as canvases are recreated.
    pub fn draw(&mut self, text: &str, color: [u8; 4], style: TextStyle, instance: M) {
        // Empty text or no alpha is assumed to be invisible.
        if text.is_empty() || color[3] == 0 {
            return;
        }

        // Save String allocation most of the time.
        // Can't use .from_key because can't implement the [`std::borrow::Borrow`] trait.
        let hash = self.buffers.hasher().hash_one((text, color, style));
        let (_, entry) = self
            .buffers
            .raw_entry_mut()
            .from_hash(hash, |(existing_text, existing_color, existing_style)| {
                existing_text.as_str() == text
                    && *existing_color == color
                    && *existing_style == style
            })
            .or_insert_with(|| ((text.to_owned(), color, style), Default::default()));

        entry.instances.push(instance)
    }
}

impl<M: TextInstance> Layer for GenTextLayer<M> {
    const ALPHA: bool = true;

    fn pre_render(&mut self, renderer: &Renderer) {
        self.buffers.retain(|id, entry| {
            entry.texture.get_or_insert_with(|| {
                // Generate textures here to avoid pipeline stall if done during rendering.
                Texture::from_text(renderer, &id.0, id.1, id.2)
            });

            // Remove textures that haven't been used in 255 (u8::MAX) frames.
            if entry.instances.is_empty() {
                if let Some(next) = entry.counter.checked_add(1) {
                    entry.counter = next;
                    true // Keep alive (was used recently).
                } else {
                    false // Destroy (wasn't used in a few seconds).
                }
            } else {
                entry.counter = 0;
                true // Keep alive (was used this frame).
            }
        });
    }
}

impl<'a, M: TextInstance> RenderLayer<M::Params<'a>> for GenTextLayer<M> {
    fn render(&mut self, renderer: &Renderer, params: M::Params<'a>) {
        // Haven't rendered text in a while.
        if self.buffers.is_empty() {
            return;
        }

        if let Some(shader) = &self.shader.bind(renderer) {
            let binding = self.geometry.bind(renderer);

            for buffers in self.buffers.values_mut() {
                if buffers.instances.is_empty() {
                    continue; // Nothing to draw.
                }

                // Shouldn't panic because texture was initialized in pre_render.
                let texture = buffers.texture.as_ref().unwrap();
                let texture_aspect = texture.aspect();
                shader.uniform("uSampler", texture);

                // Could draw multiple in a single draw call but would require binding a buffer.
                // Binding a single buffer is faster than multiple uniform calls, but we expect most
                // text to be unique.
                for instance in buffers.instances.drain(..) {
                    instance.prepare(shader, texture_aspect, params);
                    binding.draw();
                }
            }
        }
    }
}
