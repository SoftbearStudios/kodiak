// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_from_str, impl_wrapper_str, ServerNumber};
use arrayvec::ArrayString;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug, Display, Formatter, Write};
use std::num::NonZeroU8;
use std::str::FromStr;

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Encode, Decode,
)]
pub struct GameId(pub ArrayString<12>);
impl_wrapper_str!(GameId);
impl_wrapper_from_str!(GameId, ArrayString<12>);

impl GameId {
    pub fn new(s: &str) -> Self {
        debug_assert!(s.bytes().all(|b| b.is_ascii_alphanumeric()));
        Self(ArrayString::from(s).unwrap())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct InvitationId {
    pub server_number: ServerNumber,
    pub number: u16,
}

impl InvitationId {
    /// Omits B=8, G=6, I=1, O=Q=0, S=5, Z=2
    /// Omits U, A, E (prevent swear words)
    const ALPHABET: &'static [u8] = "CDFHJKLMNPRTVWXY".as_bytes();
    const ALPHABET_SIZE: u32 = Self::ALPHABET.len().ilog2();
    pub const CODE_LEN: u32 = (u8::BITS + u16::BITS) / Self::ALPHABET_SIZE;

    pub fn generate(server_number: ServerNumber) -> Self {
        use rand::Rng;
        Self {
            server_number,
            number: rand::thread_rng().gen(),
        }
    }

    pub fn server_number(self) -> ServerNumber {
        self.server_number
    }
}

impl Display for InvitationId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        debug_assert!(Self::ALPHABET.len().is_power_of_two());
        let mut n = (self.server_number.0.get() as u32) << u16::BITS | self.number as u32;
        for i in 0..Self::CODE_LEN {
            f.write_char(
                Self::ALPHABET[(n.wrapping_add(i) % Self::ALPHABET.len() as u32) as usize] as char,
            )?;
            n >>= Self::ALPHABET_SIZE;
        }
        Ok(())
    }
}

impl Serialize for InvitationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for InvitationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <String>::deserialize(deserializer).and_then(|s| {
            Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid invitation id"))
        })
    }
}

#[derive(Debug, Clone)]
pub enum InvalidInvitationId {
    Length,
    Character,
    ZeroServerNumber,
}

impl FromStr for InvitationId {
    type Err = InvalidInvitationId;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        if code.len() != Self::CODE_LEN as usize {
            return Err(InvalidInvitationId::Length);
        }
        let mut ret = 0u32;
        for (i, c) in code.bytes().enumerate().rev() {
            let raw_position = Self::ALPHABET
                .iter()
                .position(|&n| n == c)
                .ok_or(InvalidInvitationId::Character)?;
            let position = (raw_position.wrapping_sub(i) % Self::ALPHABET.len()) as u32;
            ret <<= Self::ALPHABET_SIZE;
            ret |= position;
        }
        Ok(Self {
            server_number: NonZeroU8::new((ret >> u16::BITS) as u8)
                .map(ServerNumber)
                .ok_or(InvalidInvitationId::ZeroServerNumber)?,
            number: ret as u16,
        })
    }
}
