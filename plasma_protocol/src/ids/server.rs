// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_display, impl_wrapper_from_str};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug, Display, Formatter};
use std::num::NonZeroU8;
use std::str::FromStr;
use strum::{Display, EnumIter, EnumString};

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    EnumIter,
    EnumString,
    Display,
    Hash,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub enum ServerKind {
    /// #.domain.com
    Cloud,
    /// localhost
    Local,
}

impl ServerKind {
    pub fn is_cloud(&self) -> bool {
        matches!(self, Self::Cloud)
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

impl ServerNumber {
    pub fn new(val: u8) -> Option<Self> {
        NonZeroU8::new(val).map(Self)
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct ServerNumber(pub NonZeroU8);
impl_wrapper_display!(ServerNumber);
impl_wrapper_from_str!(ServerNumber, NonZeroU8);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct ServerId {
    pub kind: ServerKind,
    pub number: ServerNumber,
}

impl ServerId {
    pub fn cloud_server_number(self) -> Option<ServerNumber> {
        if self.kind.is_cloud() {
            Some(self.number)
        } else {
            None
        }
    }
}

mod server_id_serde {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct ServerIdPlaceholder {
        kind: ServerKind,
        number: ServerNumber,
    }

    impl Serialize for ServerId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            if serializer.is_human_readable() {
                serializer.collect_str(self)
            } else {
                ServerIdPlaceholder {
                    kind: self.kind,
                    number: self.number,
                }
                .serialize(serializer)
            }
        }
    }

    impl<'de> Deserialize<'de> for ServerId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                <String>::deserialize(deserializer).and_then(|s| {
                    Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid server id"))
                })
            } else {
                ServerIdPlaceholder::deserialize(deserializer).map(|placeholder| ServerId {
                    kind: placeholder.kind,
                    number: placeholder.number,
                })
            }
        }
    }
}

impl Display for ServerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}/{}", self.kind, self.number)
    }
}

impl Debug for ServerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub enum InvalidServerId {
    MissingSlash,
    InvalidKind,
    InvalidNumber,
}

impl Display for InvalidServerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for InvalidServerId {}

impl std::str::FromStr for ServerId {
    type Err = InvalidServerId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, number) = s.split_once('/').ok_or(InvalidServerId::MissingSlash)?;
        Ok(Self {
            kind: ServerKind::from_str(kind).map_err(|_| InvalidServerId::InvalidKind)?,
            number: ServerNumber::from_str(number).map_err(|_| InvalidServerId::InvalidNumber)?,
        })
    }
}
