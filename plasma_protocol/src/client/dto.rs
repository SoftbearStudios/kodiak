// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{is_default, ArenaId, LanguageId, PlayerAlias, RankNumber, ServerNumber, VisitorId};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::Debug;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum ChatMessage {
    Raw {
        message: String,
        #[serde(default, skip_serializing_if = "is_default")]
        detected_language_id: LanguageId,
        /// If settings.language_id != detected_language_id Then english_translation Else message.
        ///
        /// Can be `None` if `message` is already english of the translation failed or was skipped.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        english_translation: Option<String>,
    },
    Welcome {
        server_number: ServerNumber,
        #[serde(default, skip_serializing_if = "is_default")]
        arena_id: ArenaId,
    },
    Join {
        /// The alias of the joiner (different from the alias of the join message sender).
        alias: PlayerAlias,
        /// Whether the alias matches the joiner's unique nickname.
        #[serde(default, skip_serializing_if = "is_default")]
        authentic: bool,
        /// The visitor id of the joiner (different from the visitor id of the join message sender).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visitor_id: Option<VisitorId>,
        /// The rank of the joiner.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rank: Option<RankNumber>,
        /// Where they joined.
        server_number: ServerNumber,
        /// Where they joined.
        #[serde(default, skip_serializing_if = "is_default")]
        arena_id: ArenaId,
    },
    /// "Either sign in or disable your VPN to chat"
    SignInOrDisableVpn,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LanguageDto {
    pub language_id: LanguageId,
    pub language_name: String,
}

/// The Leaderboard Data Transfer Object (DTO) is a single line on a leaderboard.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct LeaderboardScoreDto {
    pub alias: PlayerAlias,
    pub score: u32,
}

impl PartialOrd for LeaderboardScoreDto {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LeaderboardScoreDto {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .cmp(&other.score)
            .then_with(|| self.alias.cmp(&other.alias))
    }
}
