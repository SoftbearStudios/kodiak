// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::Vec2;

use super::{Circle, RotatedRectangle};

#[derive(Copy, Clone)]
pub enum Collider2d {
    Circle(Circle),
    RotatedRectangle(RotatedRectangle),
}

impl Collider2d {
    pub fn center(&self) -> Vec2 {
        match self {
            Self::Circle(circle) => circle.center,
            Self::RotatedRectangle(rectangle) => rectangle.center,
        }
    }

    pub fn area(&self) -> f32 {
        match self {
            Self::Circle(circle) => circle.radius.powi(2) * std::f32::consts::PI,
            Self::RotatedRectangle(rectangle) => {
                rectangle.half_size.x * rectangle.half_size.y * 4.0
            }
        }
    }

    pub fn collides(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Circle(s), Self::Circle(o)) => s.collides(o),
            (Self::Circle(s), Self::RotatedRectangle(o)) => s.rotated_rectangle(o),
            (Self::RotatedRectangle(s), Self::Circle(o)) => s.circle(o),
            (Self::RotatedRectangle(s), Self::RotatedRectangle(o)) => s.collides(o),
        }
    }
}
