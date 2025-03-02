// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{ClaimAggregation, ClaimScope, ClaimValue};
use crate::{impl_wrapper_str, serde_str, FromStrVisitor, GameId, RealmId, SkuId};
use arrayvec::ArrayString;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Copy, Clone, Hash, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ClaimName(ArrayString<28>);
impl_wrapper_str!(ClaimName);

impl ClaimName {
    pub fn new(s: &str) -> Self {
        s.parse().unwrap()
    }
}

impl FromStr for ClaimName {
    type Err = <ArrayString<28> as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ArrayString::from_str(s).map(Self)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub struct ClaimKey {
    pub name: ClaimName,
    pub aggregation: ClaimAggregation,
}

impl Display for ClaimKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.name, self.aggregation)
    }
}

#[derive(strum::Display)]
pub enum ClaimKeyError {
    #[strum(to_string = "{}")]
    Name(<ClaimName as FromStr>::Err),
    #[strum(to_string = "{}")]
    Aggregation(<ClaimAggregation as FromStr>::Err),
    #[strum(to_string = "missing slash")]
    MissingSlash,
}

impl FromStr for ClaimKey {
    type Err = ClaimKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, aggregation) = s.rsplit_once('/').ok_or(ClaimKeyError::MissingSlash)?;
        Ok(Self {
            name: ClaimName::from_str(name).map_err(ClaimKeyError::Name)?,
            aggregation: ClaimAggregation::from_str(aggregation)
                .map_err(ClaimKeyError::Aggregation)?,
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub struct GameClaimKey {
    pub game_id: GameId,
    pub key: ClaimKey,
}

impl Display for GameClaimKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.game_id, self.key)
    }
}

#[derive(strum::Display)]
pub enum GameClaimKeyError {
    #[strum(to_string = "{}")]
    GameId(<GameId as FromStr>::Err),
    #[strum(to_string = "{}")]
    Key(<ClaimKey as FromStr>::Err),
    #[strum(to_string = "missing slash")]
    MissingSlash,
}

impl FromStr for GameClaimKey {
    type Err = GameClaimKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (game_id, key) = s.split_once('/').ok_or(GameClaimKeyError::MissingSlash)?;
        Ok(Self {
            game_id: GameId::from_str(game_id).map_err(GameClaimKeyError::GameId)?,
            key: ClaimKey::from_str(key).map_err(GameClaimKeyError::Key)?,
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub struct RealmClaimKey {
    pub realm_id: RealmId,
    pub key: GameClaimKey,
}

impl Display for RealmClaimKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.realm_id, self.key)
    }
}

#[derive(strum::Display)]
pub enum RealmClaimKeyError {
    #[strum(to_string = "{}")]
    RealmId(<RealmId as FromStr>::Err),
    #[strum(to_string = "{}")]
    Key(<GameClaimKey as FromStr>::Err),
    #[strum(to_string = "missing slash")]
    MissingSlash,
}

impl FromStr for RealmClaimKey {
    type Err = RealmClaimKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (realm_id, key) = split_once_nth(s, '/', 2).ok_or(RealmClaimKeyError::MissingSlash)?;
        Ok(Self {
            realm_id: RealmId::from_str(realm_id).map_err(RealmClaimKeyError::RealmId)?,
            key: GameClaimKey::from_str(key).map_err(RealmClaimKeyError::Key)?,
        })
    }
}

/// Same as `split_at_nth_char` but don't include the character.
fn split_once_nth(s: &str, p: char, n: usize) -> Option<(&str, &str)> {
    s.match_indices(p)
        .nth(n)
        .map(|(index, _)| s.split_at(index))
        .map(|(left, right)| {
            (
                left,
                // Trim 1 character.
                &right[right.char_indices().nth(1).unwrap().0..],
            )
        })
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub struct ScopeClaimKey {
    pub scope: ClaimScope,
    pub key: ClaimKey,
}

impl Display for ScopeClaimKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.scope, self.key)
    }
}

#[derive(strum::Display)]
pub enum ScopeClaimKeyError {
    #[strum(to_string = "{}")]
    Scope(<ClaimScope as FromStr>::Err),
    #[strum(to_string = "{}")]
    Key(<ClaimKey as FromStr>::Err),
    #[strum(to_string = "missing slash")]
    MissingSlash,
}

impl FromStr for ScopeClaimKey {
    type Err = ScopeClaimKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (scope, key) = s.split_once('/').ok_or(ScopeClaimKeyError::MissingSlash)?;
        Ok(Self {
            scope: ClaimScope::from_str(scope).map_err(ScopeClaimKeyError::Scope)?,
            key: ClaimKey::from_str(key).map_err(ScopeClaimKeyError::Key)?,
        })
    }
}

impl ScopeClaimKey {
    pub fn high_score() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("high_score"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    /*
    // Global Product
    let (_, updated) = claims.merge(
        &ClaimSubset {
            claims: [(
                ScopeClaimKey::product(ClaimScope::Global, SkuId::FAST_CHAT),
                ClaimValue {
                    date_updated: NonZeroUnixMillis::now(), // now
                    date_expires: None,
                    value: 5, // quantity
                },
            )]
            .into_iter()
            .collect(),
            date_synchronized: NonZeroUnixMillis::now(), // now
        },
        GameId::Mk48, // arbitrary, because ClaimScope::Global
        realm_id, // arbitrary, because ClaimScope::Global
    );

    // Game Product
    let (_, updated) = claims.merge(
        &ClaimSubset {
            claims: [(
                ScopeClaimKey::product(ClaimScope::Game, SkuId::YELLOW_SUBMARINE),
                ClaimValue {
                    date_updated: NonZeroUnixMillis::now(), // now
                    date_expires: None,
                    value: 5, // quantity
                },
            )]
            .into_iter()
            .collect(),
            date_synchronized: NonZeroUnixMillis::now(), // now
        },
        GameId::Mk48, // game
        realm_id, // arbitrary, because ClaimScope::Game
    );
    */
    pub fn product(scope: ClaimScope, sku_id: SkuId) -> Self {
        Self {
            scope,
            key: ClaimKey {
                name: ClaimName::new(&format!("product_{sku_id}")),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    pub fn rank() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("rank"),
                aggregation: ClaimAggregation::New,
            },
        }
    }

    /// 0=opt-out, missing=1=opt-in.
    pub fn announcement_preference() -> Self {
        Self {
            scope: ClaimScope::Global,
            key: ClaimKey {
                name: ClaimName::new("announcement_preference"),
                aggregation: ClaimAggregation::New,
            },
        }
    }

    /// Play-streak:
    /// - value = # of consecutive calendar days
    /// - expiration = when streak resets
    /// - algorithm = if within 24h of expiration,
    ///   increment value and set expiration to
    ///   midnight tomorrow
    pub fn streak() -> Self {
        Self {
            scope: ClaimScope::Global,
            key: ClaimKey {
                name: ClaimName::new("streak"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    /// Number of distinct calendar days a player played a given game.
    pub fn days() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("days"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    /// # of times killed any player/bot.
    /// Currently only tracked on the public server.
    pub fn kills() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("kills"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    /// # of times killed a human player who had a considerably higher score.
    /// Currently only tracked on the public server.
    pub fn superior_kills() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("superior_kills"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    pub fn inferior_kills() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("inferior_kills"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    /// # of times victorious (in a game specific way) over player/bot.
    /// Currently only tracked on the public server.
    pub fn victories() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("victories"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    /// # of times victorious (in a game specific way) over a human player who had a considerably higher score.
    /// Currently only tracked on the public server.
    pub fn superior_victories() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("superior_victories"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }

    /// # of times victorious (in a game specific way) over a human player who had a considerably lower score.
    /// Currently only tracked on the public server.
    pub fn inferior_victories() -> Self {
        Self {
            scope: ClaimScope::Game,
            key: ClaimKey {
                name: ClaimName::new("inferior_victories"),
                aggregation: ClaimAggregation::Max,
            },
        }
    }
}

serde_str!(ClaimKey);
serde_str!(ClaimValue);
serde_str!(GameClaimKey);
serde_str!(RealmClaimKey);
serde_str!(ScopeClaimKey);
