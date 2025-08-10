// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Mat4, Vec3};

#[derive(Copy, Clone, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn mul_mat4(&self, matrix: &Mat4) -> Self {
        Self {
            origin: matrix.transform_point3(self.origin),
            direction: matrix.transform_vector3(self.direction),
        }
    }

    pub fn normalize(&self) -> Self {
        Self {
            origin: self.origin,
            // TODO: normalize_or_zero?
            direction: self.direction.normalize(),
        }
    }

    pub fn position(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}
