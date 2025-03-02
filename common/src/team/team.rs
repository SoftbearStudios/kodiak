// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Members;
use crate::bitcode::{self, *};
use crate::{PlayerAlias, PlayerId, TeamId, TeamName, TeamUpdate};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
#[fundamental]
pub struct Team<D, M> {
    pub name: Option<TeamName>,
    pub data: D,
    /// Common invariant: first is always `Some`, the leader.
    pub members: Members<M>,
    pub joiners: Vec<PlayerId>,
}

impl<D, M> Deref for Team<D, M> {
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<D, M> DerefMut for Team<D, M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<D, M: Manifestation> Team<D, M> {
    pub fn new(data: D) -> Self {
        Self {
            name: None,
            data,
            members: Default::default(),
            joiners: Default::default(),
        }
    }

    pub fn any_alive(&self) -> bool {
        self.iter().any(|m| m.manifestation.is_alive())
    }

    pub fn leader_alive(&self) -> bool {
        self.leader()
            .map(|l| l.manifestation.is_alive())
            .unwrap_or(false)
    }

    pub fn leader(&self) -> Option<&Member<M>> {
        self.members.0.get(0)
    }

    pub fn leader_mut(&mut self) -> Option<&mut Member<M>> {
        self.members.0.get_mut(0)
    }

    pub fn get(&self, player_id: PlayerId) -> Option<&Member<M>> {
        self.members
            .iter()
            .find_map(|member| (member.player_id == player_id).then_some(member))
    }

    pub fn get_mut(&mut self, player_id: PlayerId) -> Option<&mut Member<M>> {
        self.members
            .iter_mut()
            .find_map(|member| (member.player_id == player_id).then_some(member))
    }

    #[deprecated]
    pub fn add(&mut self, player_id: PlayerId, alias: PlayerAlias) {
        self.members.push(Member::new(player_id, alias));
    }

    #[deprecated]
    pub fn remove(&mut self, player_id: PlayerId) {
        self.members.remove(player_id);
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Member<M>> {
        self.members.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Member<M>> {
        self.members.iter_mut()
    }

    pub fn update(&mut self, update: TeamUpdate<M>) {
        match update {
            TeamUpdate::SetName(name) => {
                self.name = name;
            }
            TeamUpdate::AddJoiner(joiner) => {
                debug_assert!(!self.joiners.contains(&joiner));
                self.joiners.push(joiner);
            }
            TeamUpdate::RemoveJoiner(joiner) => {
                self.joiners.retain(|id| *id != joiner);
            }
            TeamUpdate::AddMember(member) => {
                self.update(TeamUpdate::<M>::RemoveJoiner(member.player_id));
                assert!(!self.members.contains(member.player_id), "duplicate member");
                self.members.push(member);
            }
            TeamUpdate::ReplaceMember(member) => {
                self.get_mut(member.player_id).unwrap().manifestation = member.manifestation;
            }
            TeamUpdate::RemoveMember(player_id) => {
                self.members.remove(player_id);
            }
        }
    }

    pub fn member_player_ids(&self) -> Vec<PlayerId> {
        self.iter().map(|m| m.player_id).collect()
    }
}

// TODO make this work on other ids such as PlayerId.
pub fn allocate_team_id<V>(map: &impl crate::actor_model::Map<TeamId, V>) -> TeamId {
    loop {
        let candidate = TeamId(rand::random());
        if crate::actor_model::Map::get(map, candidate).is_none() {
            break candidate;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Hash, Encode, Decode)]
pub struct Member<M> {
    pub player_id: PlayerId,
    pub manifestation: M,
}

pub trait Manifestation: Clone {
    const MAX_MEMBERS: usize = 6;
    const MAX_MEMBERS_AND_JOINERS: usize = Self::MAX_MEMBERS.saturating_add(2);
    /// Leader cannot voluntarily leave (as opposed to by quitting the game).
    const LEADER_CAN_LEAVE: bool = true;
    const MEMBERS_CAN_LEAVE: bool = true;
    const CAN_LEAVE_SOLO_TEAM: bool = true;
    const CAN_REUSE_SOLO_TEAM: bool = true;

    fn new(alias: PlayerAlias) -> Self;

    /// Can't be pruned when owner leaves.
    fn is_alive(&self) -> bool;

    /// Can be pruned when owner leaves.
    fn is_dead(&self) -> bool {
        !self.is_alive()
    }
}

impl Manifestation for () {
    fn new(_: PlayerAlias) -> Self {
        ()
    }

    fn is_alive(&self) -> bool {
        unreachable!("don't use *_alive");
    }
}

impl<M: Manifestation> Member<M> {
    pub const LIMIT: u8 = 6;

    pub fn new(player_id: PlayerId, alias: PlayerAlias) -> Self {
        Self {
            player_id,
            manifestation: M::new(alias),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct MemberId {
    pub team_id: TeamId,
    pub player_id: PlayerId,
}
