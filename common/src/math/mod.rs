// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod angle;
mod intersect_2d;
mod intersect_3d;
mod range;
mod rng;
mod tests;
mod x_vec2;
mod mat;

pub use self::angle::{
    deterministic_atan2, mat3_to_translation_angle, translation_angle_to_mat3, vec_to_quat,
    vec_to_yaw_pitch, Angle, AngleRepr, Cardinal4,
};
pub use self::intersect_2d::*;
pub use self::intersect_3d::*;
pub use self::range::{gen_radius, lerp, map_ranges, map_ranges_fast};
pub use self::rng::HashRng;
pub use self::x_vec2::{I16Vec2, I8Vec2, U16Vec2, U8Vec2};
pub use self::mat::normalize_scale_mat4;