// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{impl_wrapper_display, impl_wrapper_from_str};
use bitcode::{Decode, Encode};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::num::{NonZeroU32, NonZeroU64, NonZeroU8};

use rand::distributions::{Standard, WeightedIndex};
use rand::prelude::*;

pub type ClientHash = u16;

/// Cohorts 1-4 are used for A/B testing.
/// The default for existing players is cohort 1.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct CohortId(pub NonZeroU8);
impl_wrapper_display!(CohortId);
impl_wrapper_from_str!(CohortId, NonZeroU8);

impl CohortId {
    const WEIGHTS: [u8; 4] = [8, 4, 2, 1];

    pub fn new(n: u8) -> Option<Self> {
        NonZeroU8::new(n)
            .filter(|n| n.get() <= Self::WEIGHTS.len() as u8)
            .map(Self)
    }

    pub fn iter() -> impl Iterator<Item = Self> + 'static {
        (0..Self::WEIGHTS.len()).map(|i| Self::new(i as u8 + 1).unwrap())
    }
}

impl Default for CohortId {
    fn default() -> Self {
        Self::new(1).unwrap()
    }
}

impl Distribution<CohortId> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> CohortId {
        use std::sync::LazyLock;
        static DISTRIBUTION: LazyLock<WeightedIndex<u8>> =
            LazyLock::new(|| WeightedIndex::new(CohortId::WEIGHTS).unwrap());

        let n = DISTRIBUTION.sample(rng) + 1;
        debug_assert!(n > 0);
        debug_assert!(n <= CohortId::WEIGHTS.len());
        // The or default is purely defensive.
        CohortId::new(n as u8).unwrap_or_default()
    }
}

impl Serialize for CohortId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.get().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CohortId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <u8>::deserialize(deserializer)
            .and_then(|n| Self::new(n).ok_or(D::Error::custom("invalid cohort id")))
    }
}

/// Reconnection token for web socket.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct ReconnectionToken(pub NonZeroU32);
impl_wrapper_display!(ReconnectionToken);
impl_wrapper_from_str!(ReconnectionToken, NonZeroU32);

#[cfg(feature = "server")]
impl Distribution<ReconnectionToken> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> ReconnectionToken {
        ReconnectionToken(rng.gen())
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Encode, Decode,
)]
pub struct ServerToken(pub NonZeroU64);
impl_wrapper_display!(ServerToken);
impl_wrapper_from_str!(ServerToken, NonZeroU64);

#[cfg(feature = "server")]
impl Distribution<ServerToken> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> ServerToken {
        ServerToken(rng.gen())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct SessionId(pub NonZeroU64);
impl_wrapper_display!(SessionId);
impl_wrapper_from_str!(SessionId, NonZeroU64);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct SessionToken(pub NonZeroU64);
impl_wrapper_display!(SessionToken);
impl_wrapper_from_str!(SessionToken, NonZeroU64);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct SkuId(pub NonZeroU64);
impl_wrapper_display!(SkuId);
impl_wrapper_from_str!(SkuId, NonZeroU64);
