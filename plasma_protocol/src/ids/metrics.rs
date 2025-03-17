// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Display, Formatter};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

/// `LifecycleId` is used by metrics.
#[derive(
    Clone,
    Copy,
    Debug,
    Hash,
    Eq,
    PartialEq,
    Deserialize,
    EnumIter,
    EnumString,
    Display,
    Serialize,
    Encode,
    Decode,
)]
pub enum LifecycleId {
    New,
    Renewed,
}

impl LifecycleId {
    pub fn is_new(self) -> bool {
        matches!(self, Self::New)
    }

    pub fn is_renewed(self) -> bool {
        matches!(self, Self::Renewed)
    }
}

/// `PeriodId` is used by `LeaderboardScoreDto`.
#[derive(
    Clone,
    Copy,
    Debug,
    Hash,
    Eq,
    PartialEq,
    Deserialize,
    EnumIter,
    EnumString,
    Display,
    Serialize,
    Encode,
    Decode,
)]
pub enum PeriodId {
    #[serde(rename = "all")]
    AllTime = 0,
    #[serde(rename = "day")]
    Daily = 1,
    #[serde(rename = "week")]
    Weekly = 2,
}

impl PeriodId {
    pub const VARIANT_COUNT: usize = std::mem::variant_count::<Self>();
}

impl From<usize> for PeriodId {
    fn from(i: usize) -> Self {
        match i {
            0 => Self::AllTime,
            1 => Self::Daily,
            2 => Self::Weekly,
            _ => panic!("invalid index"),
        }
    }
}

impl PeriodId {
    pub fn iter() -> impl Iterator<Item = Self> {
        <Self as IntoEnumIterator>::iter()
    }
}

/// Mirrors <https://github.com/finnbear/db_ip>: `Region`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Display,
    Eq,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    EnumIter,
    EnumString,
    Serialize,
    Encode,
    Decode,
)]
pub enum RegionId {
    Africa,
    Asia,
    Europe,
    #[default]
    NorthAmerica,
    Oceania,
    SouthAmerica,
}

impl RegionId {
    /// Returns a relative distance to another region.
    /// It is not necessarily transitive.
    pub fn distance(self, other: Self) -> u8 {
        match self {
            Self::Africa => match other {
                Self::Africa => 0,
                Self::Asia => 2,
                Self::Europe => 1,
                Self::NorthAmerica => 2,
                Self::Oceania => 3,
                Self::SouthAmerica => 3,
            },
            Self::Asia => match other {
                Self::Africa => 2,
                Self::Asia => 0,
                Self::Europe => 2,
                Self::NorthAmerica => 2,
                Self::Oceania => 1,
                Self::SouthAmerica => 3,
            },
            Self::Europe => match other {
                Self::Africa => 1,
                Self::Asia => 2,
                Self::Europe => 0,
                Self::NorthAmerica => 2,
                Self::Oceania => 3,
                Self::SouthAmerica => 3,
            },
            Self::NorthAmerica => match other {
                Self::Africa => 3,
                Self::Asia => 3,
                Self::Europe => 2,
                Self::NorthAmerica => 0,
                Self::Oceania => 2,
                Self::SouthAmerica => 1,
            },
            Self::Oceania => match other {
                Self::Africa => 3,
                Self::Asia => 1,
                Self::Europe => 2,
                Self::NorthAmerica => 2,
                Self::Oceania => 0,
                Self::SouthAmerica => 3,
            },
            Self::SouthAmerica => match other {
                Self::Africa => 3,
                Self::Asia => 2,
                Self::Europe => 2,
                Self::NorthAmerica => 1,
                Self::Oceania => 2,
                Self::SouthAmerica => 0,
            },
        }
    }

    pub fn iter() -> impl Iterator<Item = Self> + 'static {
        <Self as IntoEnumIterator>::iter()
    }
}

/// Wasn't a valid region string.
#[derive(Debug)]
pub struct InvalidRegionId;

impl Display for InvalidRegionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid region id string")
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Display,
    Eq,
    PartialEq,
    Hash,
    EnumIter,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub enum UserAgentId {
    ChromeOS,
    Desktop,
    DesktopChrome,
    DesktopFirefox,
    DesktopSafari,
    Mobile,
    Spider,
    Tablet,
    // Console
}

impl UserAgentId {
    pub fn iter() -> impl Iterator<Item = Self> + 'static {
        <Self as IntoEnumIterator>::iter()
    }
}
