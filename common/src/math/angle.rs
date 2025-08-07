// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use glam::{IVec2, Mat2, Mat3, Mat4, Quat, Vec2, Vec3, Vec3Swizzles};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::f32::consts::PI;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};
use strum::EnumIter;

pub type AngleRepr = i16;

/// Represents an angle with a `i16` instead of a `f32` to get wrapping for free and be 2 bytes
/// instead of 4. All [`Angle`]'s methods and trait `impl`s are cross-platform deterministic unlike
/// [`f32::sin`], [`f32::cos`], [`f32::atan2`] etc.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Encode, Decode)]
pub struct Angle(pub AngleRepr);

impl Angle {
    pub const MAX: Self = Self(AngleRepr::MAX);
    pub const MIN: Self = Self(AngleRepr::MIN);
    pub const PI: Self = Self(AngleRepr::MAX);
    // TODO this is actually 1/65536 less than PI
    pub const PI_2: Self = Self(AngleRepr::MAX / 2);
    pub const PI_4: Self = Self(AngleRepr::MAX / 4);
    pub const ZERO: Self = Self(0);

    pub fn new() -> Self {
        Self::ZERO
    }

    /// Replacement for [`f32::atan2`]. Uses cross-platform deterministic atan2.
    pub fn from_atan2(y: f32, x: f32) -> Self {
        Self::from_radians(deterministic_atan2(y, x))
    }

    /// Replacement for [`f32::sin_cos`] (returns `vec2(cos, sin)`). Uses cross-platform
    /// deterministic sin/cos.
    #[inline]
    pub fn to_vec(self) -> Vec2 {
        let radians = self.to_radians();
        // TODO: -PI - 1 might exceed range
        Vec2::new(
            fastapprox::fast::cos(radians),
            fastapprox::fast::sin(radians),
        )
    }

    /// Replacement for `vec.y.atan2(vec.x)`. Uses cross-platform deterministic atan2.
    #[inline]
    pub fn from_vec(vec: Vec2) -> Self {
        Self::from_atan2(vec.y, vec.x)
    }

    /// Replacement for [`Mat2::from_angle`]. Uses cross-platform deterministic sin/cos.
    #[inline]
    pub fn to_mat2(self) -> Mat2 {
        let [cos, sin] = self.to_vec().to_array();
        Mat2::from_cols_array(&[cos, sin, -sin, cos])
    }

    pub fn to_mat3_x(self) -> Mat3 {
        let [cos, sin] = self.to_vec().to_array();
        Mat3::from_cols(Vec3::X, Vec3::new(0.0, cos, sin), Vec3::new(0.0, -sin, cos))
    }

    pub fn to_mat3_y(self) -> Mat3 {
        let [cos, sin] = self.to_vec().to_array();
        Mat3::from_cols(Vec3::new(cos, 0.0, -sin), Vec3::Y, Vec3::new(sin, 0.0, cos))
    }

    pub fn to_mat3_z(self) -> Mat3 {
        let [cos, sin] = self.to_vec().to_array();
        Mat3::from_cols(Vec3::new(cos, sin, 0.0), Vec3::new(-sin, cos, 0.0), Vec3::Z)
    }

    pub fn to_mat4_x(self) -> Mat4 {
        Mat4::from_mat3(self.to_mat3_x())
    }

    pub fn to_mat4_y(self) -> Mat4 {
        Mat4::from_mat3(self.to_mat3_y())
    }

    pub fn to_mat4_z(self) -> Mat4 {
        Mat4::from_mat3(self.to_mat3_z())
    }

    pub fn to_quat_x(self) -> Quat {
        let [c, s] = Self(self.0 / 2).to_vec().to_array();
        Quat::from_xyzw(s, 0.0, 0.0, c)
    }

    pub fn to_quat_y(self) -> Quat {
        let [c, s] = Self(self.0 / 2).to_vec().to_array();
        Quat::from_xyzw(0.0, s, 0.0, c)
    }

    pub fn to_quat_z(self) -> Quat {
        let [c, s] = Self(self.0 / 2).to_vec().to_array();
        Quat::from_xyzw(0.0, 0.0, s, c)
    }

    /// Converts the [`Angle`] to an `f32` in radians in the range [-PI, PI]. Opposite of
    /// [`Angle::from_radians`].
    #[inline]
    pub fn to_radians(self) -> f32 {
        self.0 as f32 * (PI / Self::PI.0 as f32)
    }

    /// Converts an `f32` in radians to an [`Angle`]. Opposite of [`Angle::to_radians`].
    #[inline]
    pub fn from_radians(radians: f32) -> Self {
        Self((radians * (Self::PI.0 as f32 / PI)) as i32 as AngleRepr)
    }

    /// Like [`Angle::from_radians`] but angles greater than `PI` are clamped to `PI`, and angles
    /// less than -`PI` are clamped to -`PI`.
    #[inline]
    pub fn saturating_from_radians(radians: f32) -> Self {
        Self((radians * (Self::PI.0 as f32 / PI)) as AngleRepr)
    }

    /// Converts the [`Angle`] to an `f32` in degrees in the range [-180, 180]. Opposite of
    /// [`Angle::from_degrees`].
    pub fn to_degrees(self) -> f32 {
        self.to_radians().to_degrees()
    }

    /// Converts an `f32` in degrees to an [`Angle`]. Opposite of [`Angle::to_degrees`].
    pub fn from_degrees(degrees: f32) -> Self {
        Self::from_radians(degrees.to_radians())
    }

    /// Converts the [`Angle`] to an `f32` in revolutions in the range [-0.5, 0.5]. Opposite of
    /// [`Angle::from_revolutions`].
    #[inline]
    pub fn to_revolutions(self) -> f32 {
        self.0 as f32 * (0.5 / Self::PI.0 as f32)
    }

    /// Converts an `f32` in revolutions to an [`Angle`].  One revolution is 360 degrees.
    #[inline]
    pub fn from_revolutions(revolutions: f32) -> Self {
        Self((revolutions * (2.0 * AngleRepr::MAX as f32)) as i32 as AngleRepr)
    }

    pub fn abs(self) -> Self {
        if self.0 == AngleRepr::MIN {
            // Don't negate with overflow.
            return Angle::MAX;
        }
        Self(self.0.abs())
    }

    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    pub fn clamp_magnitude(self, max: Self) -> Self {
        if max.0 >= 0 {
            Self(self.0.clamp(-max.0, max.0))
        } else {
            // Clamping to over 180 degrees in either direction, any angle is valid.
            self
        }
    }

    pub fn lerp(self, other: Self, value: f32) -> Self {
        self + (other - self) * value
    }

    /// Increases clockwise with straight up being 0. Output always 0..=359, never 360.
    pub fn to_bearing(self) -> u16 {
        ((Self::PI_2 - self).0 as u16 as u32 * 360 / (u16::MAX as u32 + 1)) as u16
    }

    /// N, E, S, etc.
    pub fn to_cardinal_4(self) -> Cardinal4 {
        let idx = ((self.0 as u16).wrapping_add(u16::MAX / 8)) / ((u16::MAX as u32 + 1) / 4) as u16;
        [
            Cardinal4::East,
            Cardinal4::North,
            Cardinal4::West,
            Cardinal4::South,
        ][idx as usize]
    }

    /// E, NE, SW, etc.
    pub fn to_cardinal(self) -> &'static str {
        let idx =
            ((self.0 as u16).wrapping_add(u16::MAX / 16)) / ((u16::MAX as u32 + 1) / 8) as u16;
        ["E", "NE", "N", "NW", "W", "SW", "S", "SE"][idx as usize]
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode, EnumIter)]
pub enum Cardinal4 {
    North,
    East,
    South,
    West,
}

impl Cardinal4 {
    pub fn to_vec(self) -> IVec2 {
        match self {
            Self::North => IVec2::Y,
            Self::East => IVec2::X,
            Self::South => IVec2::NEG_Y,
            Self::West => IVec2::NEG_X,
        }
    }

    /// Rotate 90 degrees clockwise.
    pub fn clockwise(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }

    /// Rotate 90 degrees counter-clockwise.
    pub fn counter_clockwise(self) -> Self {
        match self {
            Self::North => Self::West,
            Self::East => Self::North,
            Self::South => Self::East,
            Self::West => Self::South,
        }
    }

    /// Returns the corresponding angle.
    pub fn angle(self) -> Angle {
        self.into()
    }
}

impl From<Cardinal4> for Angle {
    fn from(value: Cardinal4) -> Self {
        match value {
            Cardinal4::North => Angle::PI_2,
            Cardinal4::East => Angle::ZERO,
            Cardinal4::South => -Angle::PI_2,
            Cardinal4::West => Angle::PI,
        }
    }
}

impl Default for Angle {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<Angle> for Vec2 {
    fn from(angle: Angle) -> Self {
        angle.to_vec()
    }
}

impl From<Vec2> for Angle {
    fn from(vec: Vec2) -> Self {
        Self::from_vec(vec)
    }
}

impl Add for Angle {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self(self.0.wrapping_add(other.0))
    }
}

impl AddAssign for Angle {
    fn add_assign(&mut self, other: Self) {
        self.0 = self.0.wrapping_add(other.0);
    }
}

impl Sub for Angle {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self(self.0.wrapping_sub(other.0))
    }
}

impl SubAssign for Angle {
    fn sub_assign(&mut self, other: Self) {
        self.0 = self.0.wrapping_sub(other.0);
    }
}

impl Neg for Angle {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::ZERO - self
    }
}

impl Mul<f32> for Angle {
    type Output = Self;

    fn mul(self, other: f32) -> Self::Output {
        Self((self.0 as f32 * other) as i32 as AngleRepr)
    }
}

use rand::prelude::*;
impl Distribution<Angle> for rand::distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Angle {
        Angle(rng.gen())
    }
}

impl fmt::Debug for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} degrees", self.to_degrees())
    }
}

impl Serialize for Angle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_f32(self.to_radians())
        } else {
            serializer.serialize_i16(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Angle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            <f32>::deserialize(deserializer).map(Self::from_radians)
        } else {
            <i16>::deserialize(deserializer).map(Self)
        }
    }
}

#[allow(clippy::excessive_precision)]
pub fn deterministic_atan2(y: f32, x: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        return f32::NAN;
    }
    // https://math.stackexchange.com/a/1105038
    let (ax, ay) = (x.abs(), y.abs());
    let a = ax.min(ay) / ax.max(ay);
    let s = a * a;
    let mut r = ((-0.0464964749 * s + 0.15931422) * s - 0.327622764) * s * a + a;
    if ay > ax {
        r = 1.57079637 - r;
    }
    if x < 0.0 {
        r = 3.14159274 - r;
    }
    if y < 0.0 {
        r = -r;
    }
    if r.is_finite() {
        r
    } else {
        0.0
    }
}

pub fn mat3_to_translation_angle(mat: Mat3) -> (Vec2, Angle) {
    (
        mat.transform_point2(Vec2::ZERO),
        Angle::from_vec(mat.transform_vector2(Vec2::X)),
    )
}

pub fn translation_angle_to_mat3(translation: Vec2, angle: Angle) -> Mat3 {
    Mat3::from_translation(translation).mul_mat3(&angle.to_mat3_z())
}

pub fn vec_to_quat(vec: Vec3) -> Quat {
    let (yaw, pitch) = vec_to_yaw_pitch(vec);
    yaw.to_quat_y().mul_quat(pitch.to_quat_x()).normalize()
}

pub fn vec_to_yaw_pitch(vec: Vec3) -> (Angle, Angle) {
    let yaw = -Angle::PI_2 - Angle::from_vec(vec.xz());
    let pitch = Angle::from_atan2(vec.y, vec.length());
    (yaw, pitch)
}
