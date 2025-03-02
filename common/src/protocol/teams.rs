// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::{self, *};
use crate::{Manifestation, Member, PlayerId, TeamId, TeamName};

/// For leaderboard fairness, surplus progress (e.g. points beyond current level)
/// shouldn't transfer between teams (for now, team names count as separate teams).
#[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
pub enum TeamRequest {
    /// Names the current (anonymous) team.
    Name(TeamName),
    /// Requests to join the designated team.
    Join(TeamId),
    /// Accepts the join request of the designated player.
    Accept(PlayerId),
    /// Rejects the join request of the designated player.
    Reject(PlayerId),
    /// Kicks the designated player from the current team.
    Kick(PlayerId),
    /// Removes the requesting player from their current named
    /// team and places them in a new anonymous team.
    Leave,
}

#[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
pub enum TeamUpdate<M: Manifestation> {
    /// Overwrite the name of the designated team.
    SetName(Option<TeamName>),
    /// Adds a joiner to the designated team.
    AddJoiner(PlayerId),
    /// Removes a joiner from the designated team.
    RemoveJoiner(PlayerId),
    /// Adds a new member to the designated team. Implictly clears
    /// the corresponding joiner.
    AddMember(Member<M>),
    ReplaceMember(Member<M>),
    /// Removes a member from the designated team.
    RemoveMember(PlayerId),
}
