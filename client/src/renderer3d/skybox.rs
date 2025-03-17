// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Camera3d;
use crate::renderer::{
    include_shader, DefaultRender, Layer, RenderLayer, Renderer, Shader, ShaderBinding, Texture,
    TextureType, TriangleBuffer,
};
use glam::Vec3;
use kodiak_common::glam::{vec2, Vec2};

/// A [`Layer`] that renders a skybox. Must be the first layer for now.
pub struct SkyboxLayer<S: Skybox> {
    buffer: TriangleBuffer<Vec2>,
    /// The shader/texture.
    pub skybox: S,
    shader: Shader,
}

/// Skybox texture/shader.
pub trait Skybox {
    /// The associated shader.
    fn shader(renderer: &Renderer) -> Shader;
    /// Add uniforms for rendering.
    fn prepare(&self, shader: &ShaderBinding);
}

/// A cube map for texturing a skybox.
pub struct CubeMapSkybox {
    cube_map: Texture,
}

impl Skybox for CubeMapSkybox {
    fn shader(renderer: &Renderer) -> Shader {
        include_shader!(renderer, "cubemap_skybox")
    }

    fn prepare(&self, shader: &ShaderBinding) {
        shader.uniform("uSampler", &self.cube_map);
    }
}

impl CubeMapSkybox {
    /// Creates a new [`CubeMapSkybox`] from a `cube_map` ([`Texture::typ`] == [`TextureType::Cube`]).
    pub fn new(cube_map: Texture) -> Self {
        assert_eq!(
            cube_map.typ(),
            TextureType::Cube,
            "texture must be a cube map"
        );
        Self { cube_map }
    }
}

/// A shader for texturing a skybox.
///
/// Based on "A Practical Analytic Model for Daylight"
/// aka The Preetham Model, the de facto standard analytic skydome model
/// https://www.researchgate.net/publication/220720443_A_Practical_Analytic_Model_for_Daylight
///
/// First implemented by Simon Wallner
/// http://simonwallner.at/project/atmospheric-scattering/
///
/// Improved by Martin Upitis
/// http://blenderartists.org/forum/showthread.php?245954-preethams-sky-impementation-HDR
///
/// Three.js integration by zz85 http://twitter.com/blurspline
pub struct ShaderSkybox {
    /// Overall aerosol content.
    ///
    /// 2 - clear (arctic)
    /// 3 - clear (temperate)
    /// 6 - warm, moist
    /// 10 - hazy
    pub turbidity: f32,
    /// Rayleigh scattering.
    pub rayleigh: f32,
    /// https://en.wikipedia.org/wiki/Mie_scattering
    pub mie_coefficient: f32,
    /// https://en.wikipedia.org/wiki/Mie_scattering
    pub mie_directional_g: f32,
    /// Sun position in world space.
    pub sun_position: Vec3,
    /// Exposure coefficient.
    pub exposure: f32,
}

impl Default for ShaderSkybox {
    fn default() -> Self {
        Self {
            turbidity: 2.0,
            rayleigh: 1.0,
            mie_coefficient: 0.005,
            mie_directional_g: 0.8,
            sun_position: Vec3::ZERO,
            exposure: 0.5,
        }
    }
}

impl Skybox for ShaderSkybox {
    fn shader(renderer: &Renderer) -> Shader {
        include_shader!(renderer, "shader_skybox")
    }

    fn prepare(&self, shader: &ShaderBinding) {
        shader.uniform("turbidity", self.turbidity);
        shader.uniform("rayleigh", self.rayleigh);
        shader.uniform("mieCoefficient", self.mie_coefficient);
        shader.uniform("mieDirectionalG", self.mie_directional_g);
        shader.uniform("sunPosition", self.sun_position);
        shader.uniform("exposure", self.exposure);
    }
}

impl<S: Skybox> SkyboxLayer<S> {
    /// Creates a new [`SkyboxLayer`].
    pub fn new(renderer: &Renderer, skybox: S) -> Self {
        // Create a buffer that has 1 triangle covering the whole screen.
        let mut buffer = TriangleBuffer::new(renderer);
        buffer.buffer(
            renderer,
            &[vec2(-1.0, 3.0), vec2(-1.0, -1.0), vec2(3.0, -1.0)],
            &[],
        );

        Self {
            buffer,
            skybox,
            shader: S::shader(renderer),
        }
    }
}

impl<S: Skybox> Layer for SkyboxLayer<S> {}

impl<S: Skybox> RenderLayer<&Camera3d> for SkyboxLayer<S> {
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

            self.skybox.prepare(&shader);
            self.buffer.bind(renderer).draw();

            renderer.invert_depth(false); // Set back to default: LESS.
            renderer.set_depth_mask(true);
        }
    }
}
