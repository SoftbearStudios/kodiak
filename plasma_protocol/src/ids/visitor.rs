// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_display, impl_wrapper_from_str};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::num::{NonZeroU16, NonZeroU64};

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct PlayerId(
    /// Clients are odd numbers starting at 1.
    /// Bots are even numbers starting at 2.
    pub NonZeroU16,
);
impl_wrapper_display!(PlayerId);
impl_wrapper_from_str!(PlayerId, NonZeroU16);

impl PlayerId {
    /// Gets the client number associated with this id,
    /// or [`None`] if the id is a bot.
    pub fn client_number(self) -> Option<usize> {
        self.is_client().then_some((self.0.get() as usize - 1) / 2)
    }

    /// Gets the bot number associated with this id,
    /// or [`None`] if the id is a player.
    pub fn bot_number(self) -> Option<usize> {
        self.is_bot().then_some(self.0.get() as usize / 2 - 1)
    }

    /// Gets the nth id associated with real players.
    pub fn nth_client(n: usize) -> Option<Self> {
        Some(Self(NonZeroU16::new(
            u16::try_from(n).ok()?.checked_mul(2)?.checked_add(1)?,
        )?))
    }

    /// Gets the nth id associated with bots.
    pub fn nth_bot(n: usize) -> Option<Self> {
        Some(Self(NonZeroU16::new(
            u16::try_from(n).ok()?.checked_add(1)?.checked_mul(2)?,
        )?))
    }

    /// Returns true if the id is reserved for bots.
    pub const fn is_bot(self) -> bool {
        // Bots are even numbers.
        self.0.get() % 2 == 0
    }

    pub const fn is_client(self) -> bool {
        self.0.get() % 2 == 1
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct TeamId(pub NonZeroU16);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct TeamToken(pub NonZeroU16);
impl_wrapper_display!(TeamToken);
impl_wrapper_from_str!(TeamToken, NonZeroU16);

// Signed in user.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct UserId(pub NonZeroU64);
impl_wrapper_display!(UserId);
impl_wrapper_from_str!(UserId, NonZeroU64);

// This supersedes [`PlayerId`] for persistent storage.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct VisitorId(pub NonZeroU64);
impl_wrapper_display!(VisitorId);
impl_wrapper_from_str!(VisitorId, NonZeroU64);
