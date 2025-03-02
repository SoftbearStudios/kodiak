// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod intersect_3d_tests {
    use super::{OriginAabb3d, Ray, Sphere};
    use glam::{EulerRot, Mat4, Quat, Vec3, Vec3, Vec3Swizzles};
    use rand::{thread_rng, Rng};

    #[test]
    fn origin_tests() {
        let ray = Ray {
            origin: Vec3::new(10.0, 10.0, -100.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
        };
        let origin_aabb = OriginAabb3d {
            radii: Vec3::splat(1.0),
        };
        assert!(origin_aabb.ray(&ray).is_none());
    }

    #[test]
    fn sphere_tests() {
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
                let sphere = Sphere {
                    center: Vec3::ZERO,
                    radius,
                }
                .mul_mat4(&matrix);
                let result = sphere.ray(&ray, rng.gen());
                let len = origin.xy().length();
                if (len - radius).abs() < 0.1 {
                    // Ambiguous.
                    continue;
                }
                assert_eq!(result.is_some(), len <= radius);
                if let Some(result) = result {
                    assert!(result + 0.5 > -radius - origin.z)
                }
            }
        }
    }
}
