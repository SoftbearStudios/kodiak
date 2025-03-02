// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, Decode, Encode};
use crate::{LeaderboardScoreDto, Owned, PeriodId, PlayerAlias, TeamName, VisitorId};
use serde::{Deserialize, Serialize};

/// Reason leaderboard attempts are disallowed.
///
/// This omits `TooFewPlayers` for now.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum LeaderboardCaveat {
    /// Arena was open for leaderboard, but not anymore.
    ///
    /// Clients may redirect to play on new server.
    Closing,
    /// Arena is not open for leaderboard attempts on account of server being unlisted.
    Unlisted,
    /// Arena is a temporary server (no leaderboard will be saved).
    Temporary,
}

impl LeaderboardCaveat {
    pub fn is_closing(self) -> bool {
        matches!(self, Self::Closing)
    }
}

/// Leaderboard related update from server to client.
#[derive(Clone, Debug, Encode, Decode)]
pub enum LeaderboardUpdate {
    // The leaderboard contains high score players, but not teams, for prior periods.
    Updated(PeriodId, Owned<[LeaderboardScoreDto]>),
}

/// The Liveboard Data Transfer Object (DTO) is a single line on a liveboard.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Encode, Decode)]
pub struct LiveboardDto {
    pub alias: PlayerAlias,
    pub team_name: Option<TeamName>,
    pub visitor_id: Option<VisitorId>,
    pub score: u32,
    pub authentic: bool,
}

/// Liveboard related update from server to client.
#[derive(Clone, Debug, Encode, Decode)]
pub enum LiveboardUpdate {
    /// The liveboard contains high score players in the current game.
    Updated {
        /// Latest liveboard.
        liveboard: Owned<[LiveboardDto]>,
        /// Augment with player's own score.
        your_score: Option<YourScoreDto>,
        /// Total number of players online in any tier of this server (in this realm).
        players_on_shard: u32,
        // TODO: this should be constant per game.
        shard_per_scene: bool,
        /// Total number of players online (in this realm).
        players_online: u32,
        caveat: Option<LeaderboardCaveat>,
        temporaries_available: bool,
    },
}

/// The Data Transfer Object (DTO) for your ranking and score on the liveboard.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct YourScoreDto {
    pub ranking: u16,
    pub inner: LiveboardDto,
}
