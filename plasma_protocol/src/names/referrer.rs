// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_str, slice_up_to_array_string};
use arrayvec::ArrayString;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::str::FromStr;

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct Referrer(ArrayString<16>);
impl_wrapper_str!(Referrer);

impl Referrer {
    pub fn from_hostname(mut hostname: &str, game_domain: &'static str) -> Option<Referrer> {
        if let Some(colon) = hostname.find(':') {
            hostname = &hostname[..colon];
        }
        hostname
            .split_once('.')
            .filter(|(_, d)| *d == game_domain || *d == "localhost")
            .map(|(r, _)| r)
            .filter(|&host| usize::from_str(host).is_err() && host != "www")
            .and_then(|host| Referrer::from_str(host).ok())
    }

    /// For example, given `https://foo.bar.com:1234/moo.zoo/woo.hoo` the referer will be "bar".
    pub fn new(s: &str) -> Option<Self> {
        let s = s.split_once("://").map_or(s, |(_, after)| after);
        let s = s.split('/').next().unwrap();
        let mut iter = s.rsplit('.');
        iter.next().unwrap();
        let s = if let Some(second_from_last) = iter.next() {
            // e.g. "foo.com.uk"
            matches!(second_from_last, "co" | "com")
                .then(|| iter.next())
                .flatten()
                .unwrap_or(second_from_last)
        } else {
            // e.g. localhost
            s
        };
        (!s.is_empty()).then(|| Self(slice_up_to_array_string(s)))
    }
}

impl FromStr for Referrer {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[cfg(feature = "server")]
        return Ok(Self(crate::trim_and_slice_up_to_array_string(s)));
        #[cfg(not(feature = "server"))]
        Ok(Self(slice_up_to_array_string(s)))
    }
}
