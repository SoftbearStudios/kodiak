// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Ray;
use glam::Vec3;

pub struct OriginAabb3d {
    pub radii: Vec3,
}

impl OriginAabb3d {
    pub fn ray(&self, ray: &Ray) -> Option<f32> {
        let m = ray.direction.recip();
        /*
        Precision issues:
        let n = m * ray.origin;
        let k = self.radii * m.abs();
        let t1 = -n - k;
        let t2 = -n + k;
        */
        let k = Vec3::new(
            if ray.direction.x >= 0.0 {
                self.radii.x
            } else {
                -self.radii.x
            },
            if ray.direction.y >= 0.0 {
                self.radii.y
            } else {
                -self.radii.y
            },
            if ray.direction.z >= 0.0 {
                self.radii.z
            } else {
                -self.radii.z
            },
        );
        let t1 = (-ray.origin - k) * m;
        let t2 = (-ray.origin + k) * m;

        let t_n = t1.max_element();
        let t_f = t2.min_element();
        if t_n > t_f || t_f < 0.0 {
            None
        } else {
            Some(t_n)
        }
    }
}
