// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{OriginAabb3d, Ray};
use glam::{Mat4, Quat, Vec3};

pub struct AnyBox {
    /// translation of unit AABB at origin
    //matrix: Mat4,
    /// world -> local
    inverse: Mat4,
}

impl AnyBox {
    pub fn new(center: Vec3, orientation: Quat, size: Vec3) -> Self {
        let matrix = Mat4::from_scale_rotation_translation(size, orientation, center);
        Self::from_matrix(matrix)
    }

    pub fn from_matrix(matrix: Mat4) -> Self {
        Self {
            inverse: matrix.inverse(),
        }
    }

    pub fn mul_mat4(&self, matrix: &Mat4) -> Self {
        // self.matrix = matrix.mul_mat4(self.matrix)
        // self.inverse = self.matrix.inverse()

        // TODO: optimize.
        Self {
            inverse: matrix.mul_mat4(&self.inverse.inverse()).inverse(),
        }
    }

    /// Returns the distance.
    pub fn ray(&self, ray: &Ray) -> Option<f32> {
        let ray = ray.mul_mat4(&self.inverse);
        OriginAabb3d {
            radii: Vec3::splat(0.5),
        }
        .ray(&ray)
    }
}

#[cfg(test)]
mod tests {
    use crate::{AnyBox, Ray};
    use glam::{EulerRot, Mat4, Quat, Vec3, Vec3Swizzles};

    #[test]
    fn test_ray_time() {
        let any_box = AnyBox::new(
            Vec3::new(5.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::new(1.1, 1.0, 1.0),
        );
        let ray = Ray {
            origin: Vec3::new(10.0, 0.0, 0.0),
            direction: Vec3::NEG_X,
        };
        let dist = any_box.ray(&ray).unwrap();
        let expected = 10.0 - (5.0 + 1.1 * 0.5);
        assert_eq!((dist - expected) < 0.001, "{dist} {expected}");
    }

    // cargo test test_ray_sphere
    #[test]
    fn test_ray_any_box() {
        use rand::{thread_rng, Rng};
        let mut rng = thread_rng();
        for x in -10..10 {
            for y in -10..10 {
                let radius = rng.gen_range(5.0..15.0);
                let matrix = Mat4::from_scale_rotation_translation(
                    Vec3::ONE,
                    Quat::from_euler(EulerRot::XYZ, rng.gen(), rng.gen(), rng.gen()),
                    Vec3::new(
                        rng.gen_range(-100.0..100.0),
                        rng.gen_range(-100.0..100.0),
                        rng.gen_range(-100.0..100.0),
                    ),
                );
                let origin = Vec3::new(x as f32, y as f32, -100.0);
                let ray = Ray {
                    origin,
                    direction: Vec3::Z,
                }
                .mul_mat4(&matrix);
                let any_box = AnyBox::new(Vec3::ZERO, Quat::default(), Vec3::splat(radius * 2.0))
                    .mul_mat4(&matrix);
                let result = any_box.ray(&ray).is_some();
                let len = origin.xy().abs().max_element();
                if (len - radius).abs() < 0.1 {
                    // Ambiguous.
                    continue;
                }
                assert_eq!(result, len <= radius, "{ray:?} {radius:?}");
            }
        }
    }
}
