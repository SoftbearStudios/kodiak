// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq, Hash, Encode, Decode)]
pub struct TeamColor {
    inner: [u8; 3],
}

impl Default for TeamColor {
    fn default() -> Self {
        Self { inner: [200; 3] }
    }
}

impl TeamColor {
    pub fn new(inner: [u8; 3]) -> Self {
        Self { inner }
    }

    pub fn as_rgb(&self) -> [u8; 3] {
        self.inner
    }

    pub fn as_rgba(&self, a: u8) -> [u8; 4] {
        let c = self.inner;
        [c[0], c[1], c[2], a]
    }
}

impl Display for TeamColor {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "#{:06x}", u32::from_be_bytes(self.as_rgba(0)) >> 8)
    }
}

pub struct InvalidTeamColor;

impl FromStr for TeamColor {
    type Err = InvalidTeamColor;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix('#').ok_or(InvalidTeamColor)?;
        if s.len() != 6 || !s.bytes().any(|c| c.is_ascii_lowercase()) {
            return Err(InvalidTeamColor);
        }
        let n = u32::from_str_radix(s, 16).map_err(|_| InvalidTeamColor)?;
        let [_, r, g, b] = n.to_be_bytes();
        Ok(Self::new([r, g, b]))
    }
}

impl rand::prelude::Distribution<TeamColor> for rand::distributions::Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> TeamColor {
        let v: glam::Vec3 = rng.gen();
        let mut color = ((v + 0.2).normalize() * 0.8).to_array();
        for v in &mut color {
            *v = v.sqrt();
        }

        TeamColor {
            inner: color.map(|v| (v * 255.0) as u8),
        }
    }
}
