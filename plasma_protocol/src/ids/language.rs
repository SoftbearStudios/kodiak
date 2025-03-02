// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![allow(deprecated)]

use arrayvec::ArrayString;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Display, Formatter};
use std::str::FromStr;

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct LanguageId(ArrayString<12>);

impl LanguageId {
    /// # Panics
    ///
    /// If `s.len() > 12`.
    pub fn new(s: &str) -> Self {
        Self(ArrayString::from(s).unwrap())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl FromStr for LanguageId {
    type Err = arrayvec::CapacityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

impl Default for LanguageId {
    fn default() -> Self {
        Self::new("en")
    }
}

impl Display for LanguageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        Display::fmt(&self.0, f)
    }
}
