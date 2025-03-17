// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Route;
use crate::js_hooks::{console_log, window};
use crate::{referrer, Referrer};
use std::fmt::{self, Debug, Display, Formatter};
use std::str::FromStr;
use strum::IntoEnumIterator;
use yew_router::Routable;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct PathParam<T>(pub T);

impl<T: Debug> Display for PathParam<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&format!("{:?}", self.0).to_ascii_lowercase())
    }
}

impl<T: Debug + IntoEnumIterator> FromStr for PathParam<T> {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        T::iter()
            .find(|typ| format!("{:?}", typ).eq_ignore_ascii_case(s))
            .map(Self)
            .ok_or(())
    }
}

pub fn get_real_referrer(game_domain: &'static str) -> Option<Referrer> {
    let location = window().location();
    location
        .pathname()
        .ok()
        .and_then(|pathname| Route::recognize(&pathname))
        .and_then(|route| {
            if let Route::Referrer { referrer } = route {
                console_log!("path overriding referrer to: {}", referrer);
                Some(referrer)
            } else {
                None
            }
        })
        .or_else(|| {
            location
                .hostname()
                .ok()
                .and_then(|h| Referrer::from_hostname(h.as_str(), game_domain))
                .inspect(|referrer| {
                    console_log!("host overriding referrer to: {}", referrer);
                })
        })
        .or_else(referrer)
}
