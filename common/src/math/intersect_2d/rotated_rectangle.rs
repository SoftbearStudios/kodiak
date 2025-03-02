// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Circle;
use crate::Angle;
use glam::{Vec2, Vec2Swizzles, Vec4, Vec4Swizzles};

#[derive(Copy, Clone)]
pub struct RotatedRectangle {
    pub center: Vec2,
    pub normal: Vec2,
    pub half_size: Vec2,
}

impl RotatedRectangle {
    pub fn new(position: Vec2, dimensions: Vec2, direction: Angle) -> Self {
        Self::with_normal(position, dimensions, direction.to_vec())
    }

    #[deprecated]
    pub fn center(&self) -> Vec2 {
        self.center
    }

    pub fn circle(&self, circle: &Circle) -> bool {
        circle.rotated_rectangle(self)
    }

    pub fn collides(&self, b: &Self) -> bool {
        let a = self;
        let relative_position = a.center - b.center;
        #[inline(always)]
        fn ignore(_: Vec2, _: f32, _: f32, _: Vec4) {}
        sat_collision_half(
            relative_position,
            a.normal,
            b.normal,
            a.half_size,
            b.half_size,
            &mut ignore,
        ) && sat_collision_half(
            -relative_position,
            b.normal,
            a.normal,
            b.half_size,
            a.half_size,
            &mut ignore,
        )
    }

    /// Returns (normal, depth). If the shapes overlap too much, the normal will be zero.
    pub fn collides_normal_depth(&self, b: &Self) -> Option<(Vec2, f32)> {
        let a = self;
        let relative_position = a.center - b.center;

        let mut min_penetration: (Vec2, f32) = (Vec2::ZERO, f32::INFINITY);
        let mut n = 0;
        /*
         * -----A = max     ^
         *      | :         :
         *      | :         :
         *      | v       normal
         *      | X
         *      |       X = projected[4]
         *    X |       ^
         *    ^ |   X   :
         *    : |   ^   :
         *    : |   :   :
         * -----B = min
         */
        let mut observer = |mut normal: Vec2, min: f32, max: f32, projected: Vec4| {
            if n >= 2 {
                // match `relative_position` negation.
                normal = -normal;
            }
            let projected_min = projected.min_element();
            let projected_max = projected.max_element();
            let stick_into_top = max - projected_min;
            let stick_into_bottom = projected_max - min;
            let penetration = if stick_into_top < stick_into_bottom {
                // Sticking into the top.
                (normal, stick_into_top)
            } else {
                // Sticking into the bottom.
                (-normal, stick_into_bottom)
            };
            if penetration.1 >= 0.0 && penetration.1 < min_penetration.1 {
                min_penetration = penetration;
            }
            n += 1;
        };
        if !sat_collision_half(
            relative_position,
            a.normal,
            b.normal,
            a.half_size,
            b.half_size,
            &mut observer,
        ) || !sat_collision_half(
            -relative_position,
            b.normal,
            a.normal,
            b.half_size,
            a.half_size,
            &mut observer,
        ) {
            return None;
        }
        //debug_assert!(min_penetration.1.is_finite());
        Some(min_penetration)
    }

    pub fn size(&self) -> Vec2 {
        self.half_size * 2.0
    }

    pub fn with_normal(center: Vec2, size: Vec2, normal: Vec2) -> Self {
        debug_assert!(normal.is_normalized());
        Self {
            center,
            half_size: size * 0.5, // Saves work if RotatedRectangle is used multiple times.
            normal,
        }
    }
}

/// Performs half of a SAT test (checks of one of two rectangles).
fn sat_collision_half(
    relative_position: Vec2,
    mut a_axis_normal: Vec2,
    b_axis_normal: Vec2,
    a_half_dimensions: Vec2,
    b_half_dimensions: Vec2,
    observer: &mut impl FnMut(Vec2, f32, f32, Vec4),
) -> bool {
    // Doesn't use [Vec2; 4] because half the floats would be duplicates.
    let offset_x = b_axis_normal * b_half_dimensions.x;
    let offset_y = b_axis_normal.perp() * b_half_dimensions.y;
    let other_ps = offset_x.xyxy() + offset_y.xy().extend(-offset_y.x).extend(-offset_y.y);

    // Only need to loop twice since rectangles only have 2 unique axes.
    for dimension in a_half_dimensions.to_array() {
        let dot = relative_position.dot(a_axis_normal);

        // Dimension is always positive, so min < max.
        let min = dot - dimension;
        let max = dot + dimension;
        debug_assert!(min < max, "negative dimension {min} {max} {dimension:?}");

        // Unrolled dot products are ~15% faster.
        let scaled = other_ps * a_axis_normal.xyxy();

        // vxor + vhadd
        let neg = -scaled;
        let p1 = scaled.xz() + scaled.yw();
        let p2 = neg.xz() + neg.yw();

        /*
        let projected = Vec4::new(
            all_other_ps[0].dot(a_axis_normal),
            all_other_ps[1].dot(a_axis_normal),
            all_other_ps[2].dot(a_axis_normal),
            all_other_ps[3].dot(a_axis_normal),
        );
        */
        let projected = p1.extend(p2.x).extend(p2.y);

        if projected.cmplt(Vec4::splat(min)).all() {
            return false;
        }
        if projected.cmpgt(Vec4::splat(max)).all() {
            return false;
        }

        observer(a_axis_normal, min, max, projected);

        // Start over with next axis.
        a_axis_normal = a_axis_normal.perp();
    }

    true
}

pub type SatRect = RotatedRectangle;
