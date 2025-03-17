// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Mat4, Vec2, Vec3};

use super::Ray;

pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn mul_mat4(&self, matrix: &Mat4) -> Self {
        Self {
            center: matrix.transform_point3(self.center),
            radius: matrix
                .transform_vector3(Vec3::new(self.radius, 0.0, 0.0))
                .length(),
        }
    }

    /// Returns the `t` value along the ray of the first intersection.
    pub fn ray(&self, ray: &Ray, solid: bool) -> Option<f32> {
        let oc = ray.origin - self.center;
        let b = oc.dot(ray.direction);
        let qc = oc - b * ray.direction;
        let height_2 = self.radius.powi(2) - qc.length_squared();
        if height_2 < 0.0 {
            // no intersection
            return None;
        }
        let height = height_2.sqrt();
        let t = Vec2::new(-b - height, -b + height);
        if t.y < 0.0 {
            // no intersection
            None
        } else if !solid && t.x < 0.0 {
            // ray origin inside hollow sphere, intersect on way out
            Some(t.y)
        } else {
            // ray origin outside sphere or inside solid sphere
            Some(t.x)
        }
    }
}
