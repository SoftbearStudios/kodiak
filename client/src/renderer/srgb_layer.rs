// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{
    include_shader, Antialiasing, DefaultRender, Framebuffer, Layer, RenderLayer, Renderer, Shader,
    TextureFormat, TriangleBuffer,
};
use kodiak_common::glam::{vec2, Vec2};

/// Fast approximate antialiasing.
#[cfg(feature = "renderer_fxaa")]
struct Fxaa {
    framebuffer: Framebuffer,
    shader: Shader,
}

#[cfg(feature = "renderer_fxaa")]
impl DefaultRender for Fxaa {
    fn new(renderer: &Renderer) -> Self {
        // Requires its own framebuffer because fxaa operates on non-srgb colors.
        let framebuffer = Framebuffer::new2(
            renderer,
            renderer.background_color,
            false,
            TextureFormat::Rgba { premultiply: false },
            false,
        );
        let shader = include_shader!(renderer, "fxaa");
        Self {
            framebuffer,
            shader,
        }
    }
}

#[cfg(feature = "renderer_fxaa")]
impl Fxaa {
    fn render(&mut self, renderer: &Renderer, binding: &super::TriangleBufferBinding<Vec2, u16>) {
        if let Some(shader) = self.shader.bind(renderer) {
            shader.uniform("uVP", renderer.canvas_size().as_vec2());
            shader.uniform("uInverseVP", renderer.canvas_size().as_vec2().recip());
            shader.uniform("uSampler", self.framebuffer.as_texture());
            binding.draw();
        }
    }
}

enum AntialiasingData {
    None,
    #[cfg(feature = "renderer_fxaa")]
    Fxaa(Fxaa),
    #[cfg(feature = "renderer_webgl2")]
    Msaa(Framebuffer),
}

impl AntialiasingData {
    #[cfg(feature = "renderer_fxaa")]
    fn fxaa_mut(&mut self) -> Option<&mut Fxaa> {
        match self {
            Self::Fxaa(v) => Some(v),
            _ => None,
        }
    }

    #[cfg(feature = "renderer_webgl2")]
    fn msaa(&self) -> Option<&Framebuffer> {
        match self {
            Self::Msaa(v) => Some(v),
            _ => None,
        }
    }

    #[cfg(feature = "renderer_webgl2")]
    fn msaa_mut(&mut self) -> Option<&mut Framebuffer> {
        match self {
            Self::Msaa(v) => Some(v),
            _ => None,
        }
    }
}

/// Draws its inner [`Layer`] in the [SRGB color space](https://en.wikipedia.org/wiki/SRGB). It's
/// automatically added as the root layer if the `srgb` feature is enabled.
pub(crate) struct SrgbLayer<I> {
    /// The inner [`Layer`] passed to [`new`][`Self::new`].
    pub inner: I,
    aa: AntialiasingData,
    buffer: TriangleBuffer<Vec2>,
    shader: Shader,
    texture_fb: Framebuffer,
}

impl<I: Layer + DefaultRender> DefaultRender for SrgbLayer<I> {
    fn new(renderer: &Renderer) -> Self {
        Self::with_inner(renderer, DefaultRender::new(renderer))
    }
}

impl<I: Layer> SrgbLayer<I> {
    /// Creates a new [`SrgbLayer`].
    pub(crate) fn with_inner(renderer: &Renderer, inner: I) -> Self {
        let background_color = renderer.background_color;
        let antialiasing = renderer.antialiasing;
        let depth_stencil = I::DEPTH;

        // Use builtin msaa if possible (WebGL2 only).
        let aa = match renderer.antialiasing {
            Antialiasing::None => AntialiasingData::None,
            #[cfg(feature = "renderer_fxaa")]
            Antialiasing::Fxaa => AntialiasingData::Fxaa(Fxaa::new(renderer)),
            #[cfg(feature = "renderer_webgl2")]
            Antialiasing::Msaa => AntialiasingData::Msaa(Framebuffer::new_antialiased(
                renderer,
                background_color,
                depth_stencil,
            )),
            #[cfg(not(feature = "renderer_webgl2"))]
            Antialiasing::Msaa => unimplemented!(),
        };

        // Create a buffer that has 1 triangle covering the whole screen.
        let mut buffer = TriangleBuffer::new(renderer);
        buffer.buffer(
            renderer,
            &[vec2(-1.0, 3.0), vec2(-1.0, -1.0), vec2(3.0, -1.0)],
            &[],
        );

        // For drawing to main screen.
        let shader = include_shader!(renderer, "srgb");
        let texture_fb = Framebuffer::new2(
            renderer,
            [0; 4],
            false,
            TextureFormat::Srgba { premultiply: false },
            depth_stencil && !antialiasing.is_msaa(), // msaa framebuffer has depth_stencil instead
        );

        Self {
            aa,
            buffer,
            inner,
            shader,
            texture_fb,
        }
    }
}

impl<I: Layer> Layer for SrgbLayer<I> {
    fn pre_prepare(&mut self, renderer: &Renderer) {
        self.inner.pre_prepare(renderer);
    }

    fn pre_render(&mut self, renderer: &Renderer) {
        self.inner.pre_render(renderer);
        let viewport = renderer.canvas_size();

        match &mut self.aa {
            AntialiasingData::None => (),
            #[cfg(feature = "renderer_fxaa")]
            AntialiasingData::Fxaa(fxaa) => {
                fxaa.framebuffer.set_viewport(renderer, viewport);
            }
            #[cfg(feature = "renderer_webgl2")]
            AntialiasingData::Msaa(msaa) => {
                msaa.set_viewport(renderer, viewport);
            }
        }
        self.texture_fb.set_viewport(renderer, viewport);
    }
}

impl<I: RenderLayer<P>, P> RenderLayer<P> for SrgbLayer<I> {
    fn render(&mut self, renderer: &Renderer, params: P) {
        #[cfg(feature = "renderer_webgl2")]
        let binding = self.aa.msaa_mut().map(|m| m.bind(renderer));
        #[cfg(not(feature = "renderer_webgl2"))]
        let binding = None;

        // Render directly to texture_fb if we aren't doing msaa.
        let fb = binding.unwrap_or_else(|| self.texture_fb.bind(renderer));

        // Need to clear since rendering to framebuffer (canvas has preserveDrawingBuffer: false).
        fb.clear();
        self.inner.render(renderer, params);

        drop(fb);

        // Downsample msaa results to texture_fb.
        #[cfg(feature = "renderer_webgl2")]
        if let Some(msaa) = self.aa.msaa() {
            msaa.blit_to(renderer, Some(&mut self.texture_fb));
        }

        // Capture main screen draw and render to fxaa fb.
        #[cfg(feature = "renderer_fxaa")]
        let fb = self.aa.fxaa_mut().map(|f| f.framebuffer.bind(renderer));

        // Fxaa also requires this binding so bind before shader.
        let binding = self.buffer.bind(renderer);

        // Draw to main screen. Can't do `self.texture_fb.blit_to(renderer, None);` because it
        // won't keep srgb encoding.
        if let Some(shader) = self.shader.bind(renderer) {
            shader.uniform("uSampler", self.texture_fb.as_texture());
            binding.draw();
        }

        // Draw fxaa framebuffer to main screen with fxaa applied.
        #[cfg(feature = "renderer_fxaa")]
        {
            drop(fb);
            if let Some(fxaa) = self.aa.fxaa_mut() {
                fxaa.render(renderer, &binding);
            }
        }
    }
}
