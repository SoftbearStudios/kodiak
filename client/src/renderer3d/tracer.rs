// SPDX-FileCopyrightText: 2024 Softbear, Inc.

use glam::Vec4;

use crate::glam::{vec3, Mat4, Quat, Vec3};
use crate::renderer::{
    derive_vertex, include_shader, DefaultRender, InstanceBuffer, Layer, RenderLayer, Renderer,
    Shader, TriangleBuffer,
};
use crate::renderer3d::Camera3d;

derive_vertex!(
    struct TracerInstance {
        model: Mat4,
        color: Vec4,
    }
);

/// Draws laser-like tracers.
pub struct TracerLayer {
    buffer: InstanceBuffer<TracerInstance>,
    instances: Vec<TracerInstance>,
    shader: Shader,
    triangles: TriangleBuffer<Vec3>,
}

impl DefaultRender for TracerLayer {
    fn new(renderer: &Renderer) -> Self {
        let mut triangles = TriangleBuffer::new(renderer);
        triangles.buffer(
            renderer,
            &[
                vec3(-1.0, -1.0, 1.0),
                vec3(1.0, -1.0, 1.0),
                vec3(1.0, 1.0, 1.0),
                vec3(-1.0, 1.0, 1.0),
                vec3(-1.0, -1.0, -1.0),
                vec3(1.0, -1.0, -1.0),
                vec3(1.0, 1.0, -1.0),
                vec3(-1.0, 1.0, -1.0),
            ],
            &[
                0, 1, 2, 1, 5, 6, 7, 6, 5, 4, 0, 3, 4, 5, 1, 3, 2, 6, 2, 3, 0, 6, 2, 1, 5, 4, 7, 3,
                7, 4, 1, 0, 4, 6, 7, 3,
            ],
        );
        Self {
            buffer: DefaultRender::new(renderer),
            shader: include_shader!(renderer, "tracer"),
            instances: Default::default(),
            triangles,
        }
    }
}

impl TracerLayer {
    /// Draw a tracer from point to point.
    pub fn draw(&mut self, start: Vec3, end: Vec3, radius: f32, color: Vec4) {
        let v = end - start;
        let length = v.length();
        let normal = v * (1.0 / length);
        let model = Mat4::from_translation(start)
            * Mat4::from_quat(Quat::from_rotation_arc(Vec3::Y, normal))
            * Mat4::from_scale(vec3(radius, length, radius) * 0.5)
            * Mat4::from_translation(Vec3::Y);
        self.instances.push(TracerInstance { model, color });
    }
}

impl Layer for TracerLayer {}

impl RenderLayer<&Camera3d> for TracerLayer {
    fn render(&mut self, renderer: &Renderer, camera: &Camera3d) {
        if self.instances.is_empty() {
            return;
        }
        self.buffer.buffer(renderer, &self.instances);
        self.instances.clear();

        let Some(shader) = self.shader.bind(renderer) else {
            return;
        };
        camera.prepare(&shader);
        self.buffer.bind(renderer, &self.triangles).draw();
    }
}
