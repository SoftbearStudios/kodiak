// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{ClaimKey, ClaimValue, GameClaimKey, RealmClaimKey, ScopeClaimKey};
use crate::{is_default, GameId, NonZeroUnixMillis, RealmId, UnixTime};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use strum::{Display, EnumString};

#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    Debug,
    PartialEq,
    Display,
    EnumString,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub enum ClaimScope {
    Global,
    Game,
    Realm,
}

// Pertains to a specific game and realm.
#[derive(Clone, Eq, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
// Send/receive in relation to an active player (in a specific place)
pub struct ClaimSubset {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub claims: HashMap<ScopeClaimKey, ClaimValue>,
    /// Date always chosen by the game server as the last time claims were sent to plasma.
    pub date_synchronized: NonZeroUnixMillis,
}
impl Default for ClaimSubset {
    fn default() -> Self {
        Self {
            claims: Default::default(),
            date_synchronized: NonZeroUnixMillis::MIN,
        }
    }
}

impl ClaimSubset {
    pub fn first_updated(&self) -> NonZeroUnixMillis {
        self.claims
            .values()
            .map(|v| v.date_updated)
            .min()
            .unwrap_or(NonZeroUnixMillis::MAX)
    }

    pub fn last_updated(&self) -> NonZeroUnixMillis {
        self.claims
            .values()
            .map(|v| v.date_updated)
            .max()
            .unwrap_or(NonZeroUnixMillis::MIN)
    }
}

impl Deref for ClaimSubset {
    type Target = HashMap<ScopeClaimKey, ClaimValue>;

    fn deref(&self) -> &Self::Target {
        &self.claims
    }
}

impl DerefMut for ClaimSubset {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.claims
    }
}

// Summarize public claims.
#[derive(Clone, Default, Eq, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicClaims {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_high_score: Option<NonZeroUnixMillis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_rank: Option<NonZeroUnixMillis>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub high_score: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub rank: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub streak: u64,
}

#[derive(Clone, Default, Eq, Debug, PartialEq, Serialize, Deserialize)]
// Database.
pub struct ClaimSet {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub game: HashMap<GameClaimKey, ClaimValue>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub global: HashMap<ClaimKey, ClaimValue>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub realm: HashMap<RealmClaimKey, ClaimValue>,
}

impl ClaimSet {
    /// Gets unexpired claims that pass the `filter` for a given realm, including game claims, and global claims.
    fn filtered_subset(
        &self,
        filter: impl Fn(&ScopeClaimKey, &ClaimValue) -> bool,
        date_synchronized: Option<NonZeroUnixMillis>,
        game_id: GameId,
        realm_id: RealmId,
    ) -> ClaimSubset {
        let mut claims = HashMap::new();
        for (key, value) in &self.global {
            let key = ScopeClaimKey {
                scope: ClaimScope::Global,
                key: *key,
            };
            if !filter(&key, value) {
                continue;
            }
            claims.insert(key, value.clone());
        }
        for (GameClaimKey { game_id: g, key }, value) in &self.game {
            if *g != game_id {
                continue;
            }
            let key = ScopeClaimKey {
                scope: ClaimScope::Game,
                key: *key,
            };
            if !filter(&key, value) {
                continue;
            }
            claims.insert(key, value.clone());
        }
        for (
            RealmClaimKey {
                realm_id: r,
                key: GameClaimKey { game_id: g, key },
            },
            value,
        ) in &self.realm
        {
            if *g != game_id || *r != realm_id {
                continue;
            }
            let key = ScopeClaimKey {
                scope: ClaimScope::Realm,
                key: *key,
            };
            if !filter(&key, value) {
                continue;
            }
            claims.insert(key, value.clone());
        }
        ClaimSubset {
            claims,
            date_synchronized: date_synchronized.unwrap_or(NonZeroUnixMillis::MIN),
        }
    }

    pub fn high_score(&self, game_id: GameId) -> (u64, Option<NonZeroUnixMillis>) {
        self.game
            .get(&GameClaimKey {
                game_id,
                key: ScopeClaimKey::high_score().key,
            })
            .map(|v| (v.value, Some(v.date_updated)))
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.game.is_empty() && self.global.is_empty() && self.realm.is_empty()
    }

    pub fn last_updated(&self) -> NonZeroUnixMillis {
        self.global
            .values()
            .chain(self.game.values())
            .chain(self.realm.values())
            .map(|v| v.date_updated)
            .max()
            .unwrap_or(NonZeroUnixMillis::MIN)
    }

    /// Returns change to send to client and whether to save to database.
    pub fn merge(
        &mut self,
        new: &ClaimSubset,
        game_id: GameId,
        realm_id: RealmId,
    ) -> (Option<ClaimSubset>, bool) {
        // `self` changed.
        let mut changed = false;

        let now = NonZeroUnixMillis::now();

        let mut retain = |value: &mut ClaimValue| -> bool {
            if value
                .date_expires
                .map(|expiration| expiration < now)
                .unwrap_or(false)
            {
                changed = true;
                false
            } else {
                true
            }
        };

        // Expire items to avoid improper merging.
        self.global.retain(|_, value| retain(value));
        self.game.retain(|_, value| retain(value));
        self.realm.retain(|_, value| retain(value));

        // Get recently-changed claims.
        let cutoff = new.first_updated().min(new.date_synchronized);
        let mut changed_recently = self
            .filtered_subset(
                |_, value| value.date_updated > cutoff,
                None,
                game_id,
                realm_id,
            )
            .claims
            .into_keys()
            .collect::<HashSet<_>>();

        for (scope_key, value) in &new.claims {
            let ScopeClaimKey { scope, key } = scope_key;
            let occupied = match scope {
                ClaimScope::Global => match self.global.entry(*key) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(value.clone());
                        changed = true;
                        continue;
                    }
                    Entry::Occupied(occupied) => occupied.into_mut(),
                },
                ClaimScope::Game => match self.game.entry(GameClaimKey { game_id, key: *key }) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(value.clone());
                        changed = true;
                        continue;
                    }
                    Entry::Occupied(occupied) => occupied.into_mut(),
                },
                ClaimScope::Realm => match self.realm.entry(RealmClaimKey {
                    realm_id,
                    key: GameClaimKey { game_id, key: *key },
                }) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(value.clone());
                        changed = true;
                        continue;
                    }
                    Entry::Occupied(occupied) => occupied.into_mut(),
                },
            };

            changed |= occupied.merge(value, key.aggregation);
            if occupied == value {
                changed_recently.remove(scope_key);
            } else {
                changed_recently.insert(*scope_key);
            }
        }

        let subset = Some(self.filtered_subset(
            |key, _| changed_recently.contains(&key),
            Some(new.date_synchronized),
            game_id,
            realm_id,
        ));
        (subset, changed)
    }

    pub fn public_claims(&self, game_id: GameId) -> PublicClaims {
        let (high_score, date_high_score) = self.high_score(game_id);
        let (rank, date_rank) = self.rank(game_id);
        PublicClaims {
            date_high_score,
            date_rank,
            high_score,
            rank,
            streak: self.streak(),
        }
    }

    pub fn rank(&self, game_id: GameId) -> (u64, Option<NonZeroUnixMillis>) {
        self.game
            .get(&GameClaimKey {
                game_id,
                key: ScopeClaimKey::rank().key,
            })
            .map(|v| (v.value, Some(v.date_updated)))
            .unwrap_or_default()
    }

    pub fn streak(&self) -> u64 {
        self.global
            .get(&ScopeClaimKey::streak().key)
            .map(|v| v.value)
            .unwrap_or_default()
    }

    pub fn subset(&self, game_id: GameId, realm_id: RealmId) -> ClaimSubset {
        self.filtered_subset(|_, _| true, None, game_id, realm_id)
    }
}
