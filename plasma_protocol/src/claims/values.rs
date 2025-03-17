// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::NonZeroUnixMillis;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use strum::EnumString;

#[derive(
    Copy,
    Clone,
    Hash,
    Debug,
    Default,
    Eq,
    PartialEq,
    strum::Display,
    EnumString,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub enum ClaimAggregation {
    Max,
    Min,
    #[default]
    New,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode)]
pub struct ClaimValue {
    pub date_expires: Option<NonZeroUnixMillis>,
    pub date_updated: NonZeroUnixMillis,
    pub value: u64,
}

impl ClaimValue {
    pub fn merge(&mut self, new: &Self, aggregation: ClaimAggregation) -> bool {
        let replace = match aggregation {
            ClaimAggregation::New => new.date_updated >= self.date_updated,
            ClaimAggregation::Min => new.value < self.value,
            ClaimAggregation::Max => new.value > self.value,
        };
        let mut changed = false;
        if replace && new.value != self.value {
            self.value = new.value;
            changed = true;
        }
        if replace && new.date_updated != self.date_updated {
            self.date_updated = new.date_updated;
            changed = true;
        }
        if (replace || new.date_updated >= self.date_updated)
            && self.date_expires != new.date_expires
        {
            self.date_expires = new.date_expires;
            changed = true;
        }
        changed
    }
}

impl Display for ClaimValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            date_expires,
            date_updated,
            value,
        } = self;
        if let Some(date_expires) = date_expires {
            write!(f, "{value}/{date_updated}/{date_expires}")
        } else {
            write!(f, "{value}/{date_updated}")
        }
    }
}

impl FromStr for ClaimValue {
    type Err = ClaimValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (value, dates) = s.split_once('/').ok_or(ClaimValueError::MissingSlash)?;
        if let Some((date_updated, date_expires)) = dates.split_once('/') {
            Ok(Self {
                value: u64::from_str(value).map_err(ClaimValueError::Value)?,
                date_updated: NonZeroUnixMillis::from_str(date_updated)
                    .map_err(ClaimValueError::Updated)?,
                date_expires: Some(
                    NonZeroUnixMillis::from_str(date_expires)
                        .map_err(ClaimValueError::Expiration)?,
                ),
            })
        } else {
            Ok(Self {
                value: u64::from_str(value).map_err(ClaimValueError::Value)?,
                date_updated: NonZeroUnixMillis::from_str(dates)
                    .map_err(ClaimValueError::Updated)?,
                date_expires: None,
            })
        }
    }
}

#[derive(strum::Display)]
pub enum ClaimValueError {
    #[strum(to_string = "{}")]
    Value(<u64 as FromStr>::Err),
    #[strum(to_string = "missing slash")]
    MissingSlash,
    #[strum(to_string = "{}")]
    Updated(<NonZeroUnixMillis as FromStr>::Err),
    #[strum(to_string = "{}")]
    Expiration(<NonZeroUnixMillis as FromStr>::Err),
}
