// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use arrayvec::ArrayString;
use cub::impl_wrapper_str;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct DomainName(ArrayString<28>);
impl_wrapper_str!(DomainName);

impl DomainName {
    pub fn new(s: &str) -> Option<Self> {
        ArrayString::from_str(s).ok().map(Self)
    }
}

#[derive(Debug, Clone)]
pub enum InvalidDomainName {
    MissingPart,
    InvalidCharacter(char),
    TooLong,
}

impl FromStr for DomainName {
    type Err = InvalidDomainName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for c in s.chars() {
            if !c.is_ascii_alphanumeric() && c != '.' {
                return Err(InvalidDomainName::InvalidCharacter(c));
            }
        }
        let Some((before, after)) = s.split_once('.') else {
            return Err(InvalidDomainName::MissingPart);
        };
        if before.is_empty() || after.is_empty() {
            return Err(InvalidDomainName::MissingPart);
        }
        ArrayString::from_str(s)
            .map(Self)
            .map_err(|_| InvalidDomainName::TooLong)
    }
}
