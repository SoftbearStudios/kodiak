// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod domain;
mod fmt_utils;
mod nexus;
mod player;
mod realm;
mod referrer;
mod tests;

pub use domain::DomainName;
#[cfg(feature = "server")]
pub use fmt_utils::{
    no_confusable_italics, trim_and_slice_up_to, trim_and_slice_up_to_array_string,
};
pub use fmt_utils::{slice_up_to_array_string, slice_up_to_chars};
pub use nexus::NexusPath;
pub use player::{NickName, PlayerAlias, TeamName};
pub use realm::{InvalidRealmName, RealmName};
pub use referrer::Referrer;
