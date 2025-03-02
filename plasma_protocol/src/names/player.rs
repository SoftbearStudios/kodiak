// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_from_str, impl_wrapper_str, slice_up_to_array_string, slice_up_to_chars};
use arrayvec::ArrayString;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct NickName(pub ArrayString<12>);
impl_wrapper_str!(NickName);

impl NickName {
    pub fn new(s: &str) -> Self {
        Self(slice_up_to_array_string(s))
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct PlayerAlias(ArrayString<12>);
impl_wrapper_str!(PlayerAlias);
impl_wrapper_from_str!(PlayerAlias, ArrayString<12>);

/// A player's alias (not their real name).
impl PlayerAlias {
    pub fn authority() -> Self {
        Self::new_unsanitized("Server")
    }

    pub fn capacity() -> usize {
        Self(ArrayString::new()).0.capacity()
    }

    pub fn fmt_with_team_name(self, team_name: Option<TeamName>) -> String {
        team_name
            .map(|team_name| format!("[{team_name}] {self}"))
            .unwrap_or(self.to_string())
    }

    /// Doesn't trim spaces, useful for guarding text inputs.
    pub fn new_input_sanitized(str: &str) -> Self {
        Self(slice_up_to_array_string(str))
    }

    /// Converts the string into a valid alias, which is never empty when done on the server.
    #[cfg(feature = "server")]
    pub fn new_sanitized(str: &str) -> Self {
        let str = crate::no_confusable_italics(str);
        let mut string = rustrict::Censor::from_str(&str)
            .with_censor_first_character_threshold(rustrict::Type::INAPPROPRIATE)
            .censor();

        let trimmed = rustrict::trim_whitespace(&string);

        if trimmed.starts_with('[') && trimmed.contains(']') {
            // Prevent alias confused with team name.
            string = string.replace('[', "<").replace(']', ">");
        }

        let ret = Self(crate::trim_and_slice_up_to_array_string(
            rustrict::trim_to_width(&string, 14),
        ));

        if ret.0.is_empty() {
            Self::default() // Guest
        } else {
            ret
        }
    }

    /// Good for known-good names.
    pub fn new_unsanitized(str: &str) -> Self {
        let sliced = slice_up_to_array_string(str);
        #[cfg(feature = "server")]
        debug_assert_eq!(sliced, crate::trim_and_slice_up_to_array_string(str));
        Self(sliced)
    }

    #[cfg(feature = "server")]
    pub fn sanitized(self) -> Self {
        Self::new_sanitized(self.as_str())
    }

    /// If player does not exist for some reason but we don't want to crash.
    pub fn unknown() -> Self {
        PlayerAlias::new_unsanitized("???")
    }
}

impl Default for PlayerAlias {
    fn default() -> Self {
        Self(ArrayString::from("Guest").unwrap())
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
pub struct TeamName(ArrayString<12>);
impl_wrapper_str!(TeamName);
impl_wrapper_from_str!(TeamName, ArrayString<12>);

impl TeamName {
    const MAX_CHARS: usize = 6;
    /// In units of `m`.
    #[cfg(feature = "server")]
    const MAX_WIDTH: usize = 8;

    /// Enforces `MAX_CHARS`, doesn't trim spaces, useful for guarding text inputs.
    pub fn new_input_sanitized(str: &str) -> Self {
        Self(slice_up_to_array_string(slice_up_to_chars(
            str,
            Self::MAX_CHARS,
        )))
    }

    #[cfg(feature = "server")]
    pub fn new_sanitized(str: &str) -> Self {
        let str = crate::no_confusable_italics(str);
        let string = rustrict::Censor::from_str(&str)
            .with_censor_first_character_threshold(rustrict::Type::INAPPROPRIATE)
            .take(Self::MAX_CHARS)
            .collect::<String>();

        let mut prev = string.as_str();
        loop {
            let next = rustrict::trim_whitespace(rustrict::trim_to_width(prev, Self::MAX_WIDTH))
                .trim_start_matches('[')
                .trim_end_matches(']');
            if next.len() >= prev.len() {
                // Must terminate, because next cannot get longer by trimming.
                break;
            }
            prev = next;
        }
        Self::new_unsanitized(prev)
    }

    pub fn new_unsanitized(str: &str) -> Self {
        let sliced = slice_up_to_array_string(str);
        #[cfg(feature = "server")]
        debug_assert_eq!(sliced, crate::trim_and_slice_up_to_array_string(str));
        Self(sliced)
    }
}
