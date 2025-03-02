// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use bitcode::{self, *};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU8;
use std::str::FromStr;
use strum::EnumIter;

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Ord,
    Hash,
    PartialOrd,
    Serialize,
    EnumIter,
    Deserialize,
    Encode,
    Decode,
)]
#[repr(u8)]
pub enum RankNumber {
    Rank1 = 1,
    Rank2 = 2,
    Rank3 = 3,
    Rank4 = 4,
    Rank5 = 5,
    Rank6 = 6,
}

impl RankNumber {
    pub const MAX: Self = Self::Rank6;
    const STRINGS: [&'static str; 6] = ["I", "II", "III", "IV", "V", "VI"];

    pub fn new(n: u8) -> Option<Self> {
        use RankNumber::*;
        Some(match n {
            1 => Rank1,
            2 => Rank2,
            3 => Rank3,
            4 => Rank4,
            5 => Rank5,
            6 => Rank6,
            _ => return None,
        })
    }

    pub fn get(self) -> u8 {
        self as u8
    }

    pub fn get_non_zero(self) -> NonZeroU8 {
        NonZeroU8::new(self.get()).unwrap()
    }

    pub fn as_str(self) -> &'static str {
        Self::STRINGS[(self.get() - 1) as usize]
    }
}

impl Display for RankNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub struct RankNumberError;

impl FromStr for RankNumber {
    type Err = RankNumberError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(
            Self::STRINGS
                .iter()
                .position(|s2| *s2 == s)
                .ok_or(RankNumberError)? as u8
                + 1,
        )
        .unwrap())
    }
}
