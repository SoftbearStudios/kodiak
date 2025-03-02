// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use crate::TeamId;
use arrayvec::ArrayVec;

#[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
pub struct JoinedStatus {
    /// Player's current team.
    pub team_id: TeamId,
    /// Teams that player requested to join.
    ///
    /// Invariant: If `team_id` is a named team, this is empty.
    pub joins: ArrayVec<TeamId, { Self::MAX_JOINS }>,
}

impl JoinedStatus {
    pub const MAX_JOINS: usize = 3;
}

#[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
pub enum JoinUpdate {
    /// Go back to spawning.
    Quit,
    /// Join team and clear joins.
    Join(TeamId),
    /// Add new join request.
    AddJoin(TeamId),
    /// Remove existing join request.
    RemoveJoin(TeamId),
}

#[derive(Clone, Debug, Default, PartialEq, Hash, Encode, Decode)]
pub enum PlayerStatus {
    #[default]
    Spawning,
    Joined(JoinedStatus),
}

impl PlayerStatus {
    pub fn is_spawning(&self) -> bool {
        matches!(self, Self::Spawning)
    }

    pub fn is_joined(&self) -> bool {
        matches!(self, Self::Joined { .. })
    }

    pub fn team_id(&self) -> Option<TeamId> {
        if let Self::Joined(joined) = self {
            Some(joined.team_id)
        } else {
            None
        }
    }

    pub fn joins(&self) -> Option<ArrayVec<TeamId, { JoinedStatus::MAX_JOINS }>> {
        if let Self::Joined(joined) = self {
            Some(joined.joins.clone())
        } else {
            None
        }
    }

    pub fn player_update(&mut self, update: &JoinUpdate) {
        match update {
            JoinUpdate::Quit => {
                *self = PlayerStatus::Spawning;
            }
            &JoinUpdate::Join(team_id) => {
                *self = PlayerStatus::Joined(JoinedStatus {
                    team_id,
                    joins: Default::default(),
                });
            }
            &JoinUpdate::AddJoin(team_id) => {
                if let PlayerStatus::Joined(joined) = self {
                    joined.joins.push(team_id);
                } else {
                    unreachable!();
                }
            }
            &JoinUpdate::RemoveJoin(team_id) => {
                if let PlayerStatus::Joined(joined) = self {
                    joined.joins.retain(|id| *id != team_id);
                } else {
                    unreachable!();
                }
            }
        }
    }
}
