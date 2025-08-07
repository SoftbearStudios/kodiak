// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use rand::seq::IteratorRandom;
use rand::Rng;

use super::{Manifestation, Member, MemberId, PlayerStatus, Team};
use crate::{
    JoinUpdate, JoinedStatus, PlayerAlias, PlayerId, TeamId, TeamName, TeamRequest, TeamUpdate,
};
use std::collections::HashSet;

/// Implemented by the server in an entity-player-team actor model.
pub trait PlayerTeamModel {
    type TeamData: Clone;
    type Manifestation: Manifestation;

    /// e.g. if manifestation is an entity, put it on the designated team. Also called
    /// when team name is changed.
    fn swap_manifestation(
        &mut self,
        member_id: MemberId,
        manifestation: &mut Self::Manifestation,
        new_team_id: TeamId,
        new_team_data: &Self::TeamData,
    );
    fn delete_manifestation(&mut self, member_id: MemberId, manifestation: &Self::Manifestation);

    fn has_player(&self, player_id: PlayerId) -> bool;
    /// Get the current status of the designated player.
    fn get_player_status(&self, player_id: PlayerId) -> PlayerStatus;
    /// Update the designated player.
    fn update_player(&mut self, player_id: PlayerId, update: JoinUpdate);
    /// Gets all player ids.
    fn player_ids(&self) -> impl Iterator<Item = PlayerId> + '_;

    /// Create a new, empty, anonymous team.
    fn create_team(&mut self) -> TeamId;
    /// Returns `true` iff the team exists (for validating player input).
    fn has_team(&self, team_id: TeamId) -> bool;
    /// Get the current state of the designated team.
    fn get_team(&self, team_id: TeamId) -> &Team<Self::TeamData, Self::Manifestation>;
    /// Updates the designated team.
    fn update_team(&mut self, team_id: TeamId, update: TeamUpdate<Self::Manifestation>);
    /// Deletes the designated team.
    ///
    /// The caller is responsible for clearning joins from joining players and ensuring there
    /// are no surviving manifestations.
    fn delete_team(&mut self, team_id: TeamId);
    /// Gets all team ids.
    fn team_ids(&self) -> impl Iterator<Item = TeamId> + '_;
    /// Gets all teams (don't need to override).
    fn teams(&self) -> impl Iterator<Item = (TeamId, &Team<Self::TeamData, Self::Manifestation>)> {
        self.team_ids()
            .map(|team_id| (team_id, self.get_team(team_id)))
    }

    /// After calling, player is guaranteed to be part of a team (previous, invited,
    /// or new, in that order of priority).
    fn prepare_spawn_team(
        &mut self,
        player_id: PlayerId,
        alias: PlayerAlias,
        invitation_player_id: Option<PlayerId>,
    ) -> TeamId {
        self.get_player_status(player_id)
            .team_id()
            .inspect(|team_id| assert!(self.has_team(*team_id)))
            .unwrap_or_else(|| {
                let new_member = Member::new(player_id, alias);
                let team_id = if let Some(invitation_player_id) = invitation_player_id
                    && self.has_player(invitation_player_id)
                    && let Some(team_id) = self.get_player_status(invitation_player_id).team_id()
                    && let team = self.get_team(team_id)
                    && team.name.is_some()
                    && team.members.len() < <Self::Manifestation as Manifestation>::MAX_MEMBERS
                {
                    team_id
                } else {
                    self.create_team()
                };
                self.update_team(team_id, TeamUpdate::AddMember(new_member));
                self.update_player(player_id, JoinUpdate::Join(team_id));
                team_id
            })
    }

    fn player_quit_game(&mut self, player_id: PlayerId) {
        let PlayerStatus::Joined(JoinedStatus { team_id, joins }) =
            self.get_player_status(player_id)
        else {
            return;
        };
        let team = self.get_team(team_id);
        let delete_team = team.members.len() == 1;
        assert!(team.name.is_some() || delete_team);
        let joiners = team.joiners.clone();
        let member = team.get(player_id).unwrap();
        self.delete_manifestation(
            MemberId { team_id, player_id },
            &member.manifestation.clone(),
        );

        /*
        self.world.mut_sector(
                entity_id.sector_id,
                SectorEdit::Netquel(MemberId { team_id, player_id }),
            );
         */

        if delete_team {
            for joiner in joiners {
                self.update_player(joiner, JoinUpdate::RemoveJoin(team_id));
            }
            self.delete_team(team_id);
        } else {
            self.update_team(team_id, TeamUpdate::RemoveMember(player_id));
        }
        self.update_player(player_id, JoinUpdate::Quit);
        for join in joins {
            self.update_team(join, TeamUpdate::RemoveJoiner(player_id));
        }
    }

    /// Random, possibly-invalid team request.
    #[cfg(feature = "server")]
    fn random_team_request(
        &self,
        rng: &mut impl Rng,
        random_team_name: fn() -> TeamName,
    ) -> TeamRequest {
        let random_player_id = self
            .player_ids()
            .chain(std::iter::once(PlayerId(rng.gen())))
            .choose(rng)
            .unwrap();
        let random_team_id = self
            .team_ids()
            .chain(std::iter::once(TeamId(rng.gen())))
            .choose(rng)
            .unwrap();
        match rng.gen_range(0..61) {
            0..=19 => TeamRequest::Accept(random_player_id),
            20..=29 => TeamRequest::Join(random_team_id),
            30..=39 => TeamRequest::Kick(random_player_id),
            40..=49 => TeamRequest::Name(random_team_name()),
            50..=59 => TeamRequest::Reject(random_player_id),
            _ => TeamRequest::Leave,
        }
    }

    fn handle_team_request(
        &mut self,
        req_player_id: PlayerId,
        request: TeamRequest,
    ) -> Result<(), &'static str> {
        // Put this here so kicking can call leaving even if leaving isn't permitted.
        if !Self::Manifestation::MEMBERS_CAN_LEAVE && matches!(request, TeamRequest::Leave) {
            return Err("members can't leave");
        }
        self.impl_handle_team_request(req_player_id, request)
    }

    fn impl_handle_team_request(
        &mut self,
        req_player_id: PlayerId,
        request: TeamRequest,
    ) -> Result<(), &'static str> {
        match request {
            TeamRequest::Name(new_name) => {
                let new_name = TeamName::new_sanitized(new_name.as_str());
                if new_name.is_empty() {
                    return Err("empty team name");
                }
                let PlayerStatus::Joined(JoinedStatus {
                    team_id: req_team_id,
                    joins,
                }) = self.get_player_status(req_player_id)
                else {
                    return Err("not in team");
                };

                let req_team = self.get_team(req_team_id);
                if req_team.leader().map(|l| l.player_id) != Some(req_player_id) {
                    return Err("unauthorized");
                }
                if req_team.name.is_some() {
                    return Err("team already named");
                }
                let new_team_data = req_team.data.clone();
                assert_eq!(req_team.members.len(), 1);
                assert!(req_team.joiners.is_empty());
                let joins = joins.clone();
                let mut leader = req_team.leader().unwrap().clone();
                self.swap_manifestation(
                    MemberId {
                        team_id: req_team_id,
                        player_id: req_player_id,
                    },
                    &mut leader.manifestation,
                    req_team_id,
                    &new_team_data,
                );
                self.update_team(req_team_id, TeamUpdate::ReplaceMember(leader));
                for join in joins {
                    // Now that player is on a named team, they are no longer requesting to join teams.
                    self.update_player(req_player_id, JoinUpdate::RemoveJoin(join));
                    self.update_team(join, TeamUpdate::RemoveJoiner(req_player_id));
                }
                self.update_team(req_team_id, TeamUpdate::SetName(Some(new_name)));
            }
            TeamRequest::Join(join_team_id) => {
                let PlayerStatus::Joined(JoinedStatus {
                    team_id: req_team_id,
                    joins,
                }) = self.get_player_status(req_player_id)
                else {
                    return Err("not in team");
                };

                let req_team = self.get_team(req_team_id);
                if req_team.name.is_some() {
                    return Err("already on named team");
                }
                if joins.contains(&join_team_id) {
                    return Err("already joining that team");
                }
                if !self.has_team(join_team_id) {
                    return Err("no such team");
                }
                let join_team = self.get_team(join_team_id);
                if join_team.name.is_none() {
                    return Err("cannot join un-named team");
                }
                if (join_team.members.len() + join_team.joiners.len())
                    >= Self::Manifestation::MAX_MEMBERS_AND_JOINERS
                {
                    return Err("too many joiners already");
                }
                if joins.is_full() {
                    let expired_join_team_id = joins[0];
                    self.update_team(
                        expired_join_team_id,
                        TeamUpdate::RemoveJoiner(req_player_id),
                    );
                    self.update_player(req_player_id, JoinUpdate::RemoveJoin(expired_join_team_id));
                }
                self.update_player(req_player_id, JoinUpdate::AddJoin(join_team_id));
                self.update_team(join_team_id, TeamUpdate::AddJoiner(req_player_id));
            }
            TeamRequest::Accept(accept_player_id) => {
                let Some(req_team_id) = self.get_player_status(req_player_id).team_id() else {
                    return Err("not in team");
                };

                let req_team = self.get_team(req_team_id);
                if req_team.leader().map(|l| l.player_id) != Some(req_player_id) {
                    return Err("unauthorized");
                }
                if !req_team.joiners.contains(&accept_player_id) {
                    return Err("player was not joining");
                }
                if req_team.members.len() >= Self::Manifestation::MAX_MEMBERS {
                    return Err("team full");
                }
                let new_team_data = req_team.data.clone();

                // Player was joining, so must exist.
                let old_status = self.get_player_status(accept_player_id);
                let PlayerStatus::Joined(JoinedStatus {
                    team_id: old_team_id,
                    joins: old_joins,
                }) = old_status
                else {
                    unreachable!("player was joining so must be in an anonymous team");
                };
                assert!(old_joins.contains(&req_team_id));
                let old_team_id = old_team_id;
                let old_team = self.get_team(old_team_id);
                assert!(old_team.name.is_none());
                assert_eq!(old_team.members.len(), 1);
                assert!(old_team.joiners.is_empty());
                let old_member = old_team.get(accept_player_id).unwrap();
                let mut manifestation = old_member.manifestation.clone();
                for join in old_joins.clone() {
                    // Updating the player later on will remove the joins.
                    self.update_team(join, TeamUpdate::RemoveJoiner(accept_player_id));
                }
                self.swap_manifestation(
                    MemberId {
                        team_id: old_team_id,
                        player_id: accept_player_id,
                    },
                    &mut manifestation,
                    req_team_id,
                    &new_team_data,
                );
                self.update_player(accept_player_id, JoinUpdate::Join(req_team_id));
                self.update_team(
                    req_team_id,
                    TeamUpdate::AddMember(Member {
                        player_id: accept_player_id,
                        manifestation,
                    }),
                );
                // team had one player, but don't bother removing with
                // `self.update_team(old_team_id, TeamUpdate::RemoveMember(accept_player_id));`
                self.delete_team(old_team_id);
            }
            TeamRequest::Reject(reject_player_id) => {
                let Some(req_team_id) = self.get_player_status(req_player_id).team_id() else {
                    return Err("not in team");
                };

                let req_team = self.get_team(req_team_id);
                if req_team.leader().map(|l| l.player_id) != Some(req_player_id) {
                    return Err("unauthorized");
                }
                if !req_team.joiners.contains(&reject_player_id) {
                    return Err("player was not joining");
                }
                assert!(self
                    .get_player_status(reject_player_id)
                    .joins()
                    .unwrap()
                    .contains(&req_team_id));
                self.update_player(reject_player_id, JoinUpdate::RemoveJoin(req_team_id));
                self.update_team(req_team_id, TeamUpdate::RemoveJoiner(reject_player_id));
            }
            TeamRequest::Kick(kick_player_id) => {
                let Some(req_team_id) = self.get_player_status(req_player_id).team_id() else {
                    return Err("not in team");
                };

                let req_team = self.get_team(req_team_id);
                if req_team.leader().map(|l| l.player_id) != Some(req_player_id) {
                    return Err("unauthorized");
                }
                if kick_player_id == req_player_id {
                    return Err("cannot kick self");
                }
                if !req_team.members.contains(kick_player_id) {
                    return Err("cannot kick non-member");
                }
                // We know the team has a name since anonymous teams only have one member, so there
                // is no one to kick.
                // TODO: investigate whether the error ought to be handled.
                let _ = self.impl_handle_team_request(kick_player_id, TeamRequest::Leave);
            }
            TeamRequest::Leave => {
                let old_status = self.get_player_status(req_player_id);
                let PlayerStatus::Joined(JoinedStatus {
                    team_id: old_team_id,
                    joins: old_joins,
                }) = &old_status
                else {
                    return Err("not in team");
                };
                let old_team_id = *old_team_id;

                let old_team = self.get_team(old_team_id);
                if old_team.name.is_none() {
                    return Err("cannot leave anonymous team");
                }
                assert!(old_joins.is_empty());

                let solo = old_team.members.len() == 1;
                if !<Self::Manifestation as Manifestation>::LEADER_CAN_LEAVE
                    && old_team.leader().unwrap().player_id == req_player_id
                {
                    return Err("leader cannot leave");
                } else if solo && <Self::Manifestation as Manifestation>::CAN_REUSE_SOLO_TEAM {
                    if !<Self::Manifestation as Manifestation>::CAN_LEAVE_SOLO_TEAM {
                        return Err("cannot leave solo team");
                    }

                    // Re-use old team as solo team.
                    for joiner in old_team.joiners.clone() {
                        self.update_player(joiner, JoinUpdate::RemoveJoin(old_team_id));
                        self.update_team(old_team_id, TeamUpdate::RemoveJoiner(joiner));
                    }

                    self.update_team(old_team_id, TeamUpdate::SetName(None));
                } else {
                    // Create new solo team for self.
                    let old_member = old_team.get(req_player_id).unwrap();
                    let old_joiners = old_team.joiners.clone();
                    let mut manifestation = old_member.manifestation.clone();

                    let new_team_id = self.create_team();
                    let new_team_data = self.get_team(new_team_id).data.clone();
                    self.swap_manifestation(
                        MemberId {
                            team_id: old_team_id,
                            player_id: req_player_id,
                        },
                        &mut manifestation,
                        new_team_id,
                        &new_team_data,
                    );
                    // Note: don't have to clear joins since player was on a named team and so has none.
                    self.update_player(req_player_id, JoinUpdate::Join(new_team_id));
                    if solo {
                        // Must delete old team.
                        for joiner in old_joiners {
                            self.update_player(joiner, JoinUpdate::RemoveJoin(old_team_id));
                            self.update_team(old_team_id, TeamUpdate::RemoveJoiner(joiner));
                        }
                        self.delete_team(old_team_id);
                    } else {
                        self.update_team(old_team_id, TeamUpdate::RemoveMember(req_player_id));
                    }
                    self.update_team(
                        new_team_id,
                        TeamUpdate::AddMember(Member {
                            player_id: req_player_id,
                            manifestation,
                        }),
                    );
                }
            }
        }
        Ok(())
    }

    fn assert_valid_teams(
        &self,
        player_ids: impl IntoIterator<Item = PlayerId>,
        team_ids: impl IntoIterator<Item = TeamId>,
    ) {
        for player_id in player_ids {
            assert!(self.has_player(player_id));
            let status = self.get_player_status(player_id);
            match status {
                PlayerStatus::Joined(JoinedStatus { team_id, joins }) => {
                    assert!(self.has_team(team_id));
                    let team = self.get_team(team_id);
                    assert!(team.members.contains(player_id));
                    let _member = team.get(player_id).unwrap();
                    if team.name.is_some() {
                        assert!(joins.is_empty());
                    } else {
                        for join_team_id in joins {
                            let join_team = self.get_team(join_team_id);
                            assert!(join_team.joiners.contains(&player_id));
                            assert!(join_team.name.is_some());
                        }
                    }
                }
                _ => {}
            }
        }

        for team_id in team_ids {
            assert!(self.has_team(team_id));
            let team = self.get_team(team_id);
            for member in team.iter() {
                let member_player_status = self.get_player_status(member.player_id);
                let PlayerStatus::Joined(JoinedStatus {
                    team_id: joined_team_id,
                    joins: _,
                    ..
                }) = member_player_status
                else {
                    unreachable!();
                };
                assert_eq!(team_id, joined_team_id);
            }
            if team.name.is_none() {
                assert!(team.members.len() <= 1);
                assert!(team.joiners.is_empty());
            } else {
                assert!(team.members.len() <= Self::Manifestation::MAX_MEMBERS);
                assert!(
                    team.members.len() + team.joiners.len()
                        <= Self::Manifestation::MAX_MEMBERS_AND_JOINERS
                );
                assert_eq!(
                    team.members.len(),
                    team.iter()
                        .map(|m| m.player_id)
                        .collect::<HashSet<_>>()
                        .len()
                );
                assert_eq!(
                    team.joiners.len(),
                    team.joiners.iter().collect::<HashSet<_>>().len()
                );
                for &joiner_player_id in &team.joiners {
                    let joiner_player_status = self.get_player_status(joiner_player_id);
                    let PlayerStatus::Joined(joined) = joiner_player_status else {
                        unreachable!();
                    };
                    assert!(joined.joins.contains(&team_id));
                }
            }
        }
    }
}
