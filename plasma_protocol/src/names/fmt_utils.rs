// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use arrayvec::ArrayString;

#[cfg(feature = "server")]
pub fn no_confusable_italics(s: &str) -> std::borrow::Cow<'_, str> {
    let lower = '𝘢'..='𝘻';
    let upper = '𝘈'..='𝘡';

    if s.chars().any(|c| lower.contains(&c) || upper.contains(&c)) {
        std::borrow::Cow::Owned(
            s.chars()
                .map(|c| {
                    if lower.contains(&c) {
                        char::from_u32(c as u32 - *lower.start() as u32 + 'a' as u32).unwrap()
                    } else if upper.contains(&c) {
                        char::from_u32(c as u32 - *upper.start() as u32 + 'A' as u32).unwrap()
                    } else {
                        c
                    }
                })
                .collect(),
        )
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

fn slice_up_to_bytes(s: &str, bytes: usize) -> &str {
    let mut idx = bytes;
    while !s.is_char_boundary(idx) {
        idx -= 1;
    }
    &s[..idx]
}

pub fn slice_up_to_chars(s: &str, max: usize) -> &str {
    &s[0..s
        .char_indices()
        .nth(max)
        .map(|(idx, _)| idx)
        .unwrap_or(s.len())]
}

pub fn slice_up_to_array_string<const CAPACITY: usize>(s: &str) -> ArrayString<CAPACITY> {
    ArrayString::from(slice_up_to_bytes(s, CAPACITY)).unwrap()
}

#[cfg(feature = "server")]
pub fn trim_and_slice_up_to(s: &str, bytes: usize) -> &str {
    slice_up_to_bytes(rustrict::trim_whitespace(s), bytes)
}

#[cfg(feature = "server")]
pub fn trim_and_slice_up_to_array_string<const CAPACITY: usize>(s: &str) -> ArrayString<CAPACITY> {
    ArrayString::from(trim_and_slice_up_to(s, CAPACITY)).unwrap()
}
