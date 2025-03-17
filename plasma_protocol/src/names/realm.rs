// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_str, slice_up_to_array_string};
use arrayvec::ArrayString;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct RealmName(pub ArrayString<12>);
impl_wrapper_str!(RealmName);

impl RealmName {
    pub fn new(s: &str) -> Self {
        Self(slice_up_to_array_string(s))
    }
}

#[derive(Debug, Clone)]
pub enum InvalidRealmName {
    Www,
    TooLong,
}

impl FromStr for RealmName {
    type Err = InvalidRealmName;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "www" {
            Err(InvalidRealmName::Www)
        } else {
            Ok(Self(
                ArrayString::from_str(s).map_err(|_| InvalidRealmName::TooLong)?,
            ))
        }
    }
}
