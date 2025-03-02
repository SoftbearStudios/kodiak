// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::RotatedRectangle;
use glam::{Mat2, Vec2};

#[derive(Copy, Clone)]
pub struct Circle {
    pub center: Vec2,
    pub radius: f32,
}

impl Circle {
    pub fn new(center: Vec2, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn collides(&self, other: &Self) -> bool {
        self.center.distance_squared(other.center) <= (self.radius + other.radius).powi(2)
    }

    pub fn contains(&self, point: Vec2) -> bool {
        self.center.distance_squared(point) <= self.radius.powi(2)
    }

    pub fn rotated_rectangle(&self, rect: &RotatedRectangle) -> bool {
        let Vec2 { x: cos, y: sin } = rect.normal;
        let matrix = Mat2::from_cols(Vec2::new(cos, sin), Vec2::new(-sin, cos)).inverse();
        let center = matrix.mul_vec2(self.center - rect.center);
        let clamped = center.clamp(-rect.half_size, rect.half_size);
        center.distance_squared(clamped) <= self.radius.powi(2)
    }
}
