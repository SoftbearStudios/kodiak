// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_display, impl_wrapper_from_str};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug, Display, Formatter};
use std::num::NonZeroU8;
use std::str::FromStr;

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub struct InstanceNumber(pub u8);
impl_wrapper_display!(InstanceNumber);
impl_wrapper_from_str!(InstanceNumber, u8);

impl InstanceNumber {
    pub fn new(n: u8) -> Self {
        Self(n)
    }
}

#[derive(Copy, Clone, Default, Eq, PartialEq, Hash, Encode, Decode, Ord, PartialOrd)]
pub struct SceneId {
    pub tier_number: Option<TierNumber>,
    pub instance_number: InstanceNumber,
}

impl SceneId {
    pub fn new(tier_number: Option<TierNumber>, instance_number: InstanceNumber) -> Self {
        Self {
            instance_number,
            tier_number,
        }
    }
}

mod scene_id_serde {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct SceneIdPlaceholder {
        pub instance_number: InstanceNumber,
        pub tier_number: Option<TierNumber>,
    }

    impl Serialize for SceneId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            if serializer.is_human_readable() {
                serializer.collect_str(self)
            } else {
                SceneIdPlaceholder {
                    instance_number: self.instance_number,
                    tier_number: self.tier_number,
                }
                .serialize(serializer)
            }
        }
    }

    impl<'de> Deserialize<'de> for SceneId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                <String>::deserialize(deserializer).and_then(|s| {
                    Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid scene id"))
                })
            } else {
                SceneIdPlaceholder::deserialize(deserializer).map(|placeholder| SceneId {
                    instance_number: placeholder.instance_number,
                    tier_number: placeholder.tier_number,
                })
            }
        }
    }
}

impl Display for SceneId {
    // The default scene ID is "0" After that comes "1", "2", etc.
    // If there are tiers, then comes "A", "A1" .. "B", "B1", etc.
    // (Note that "A" is equivalent to "A0", "B" to "B0", etc.)
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        if let Some(tier_number) = &self.tier_number {
            Display::fmt(tier_number, f)?;
            // if self.instance_number != Default::default() {
            let _ = Display::fmt(&self.instance_number, f)?;
            // }
        } else {
            let _ = Display::fmt(&self.instance_number, f)?;
        }
        Ok(())
    }
}

impl Debug for SceneId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub enum InvalidSceneId {
    Empty,
    InvalidInstanceNumber,
}

impl Display for InvalidSceneId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for InvalidSceneId {}

impl FromStr for SceneId {
    type Err = InvalidSceneId;

    // The default scene ID is "0" After that comes "1", "2", etc.
    // If there are tiers, then comes "A0", "A1" .. "B0", "B1", etc.
    // (Note that "A" is equivalent to "A0", "B" to "B0", etc.)
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(InvalidSceneId::Empty);
        }
        let (tier_number, s) = if s.as_bytes()[0].is_ascii_uppercase() {
            (Some(TierNumber::from_str(&s[..1]).unwrap()), &s[1..])
        } else {
            (None, s)
        };
        let instance_number = if s.is_empty() {
            Default::default()
        } else {
            InstanceNumber::from_str(s).map_err(|_| InvalidSceneId::InvalidInstanceNumber)?
        };

        Ok(Self::new(tier_number, instance_number))
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct TierNumber(pub NonZeroU8);

impl TierNumber {
    pub fn new(n: u8) -> Option<Self> {
        NonZeroU8::new(n).map(Self)
    }
}

impl Display for TierNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let letter = if self.0 < NonZeroU8::new(26).unwrap() {
            ('A' as u8 - 1u8 + u8::from(self.0)) as char
        } else {
            'Z'
        };
        Display::fmt(&letter, f)
    }
}

#[derive(Debug, Clone)]
pub struct InvalidTierNumber;

impl Display for InvalidTierNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for InvalidTierNumber {}

impl FromStr for TierNumber {
    type Err = InvalidTierNumber;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase() {
            let n = 1u8 + s.as_bytes()[0] - b'A';
            Ok(TierNumber::new(n).unwrap())
        } else {
            Err(InvalidTierNumber)
        }
    }
}
