// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::shard_context::LiveboardCohort;
use super::ShardContextProvider;
use crate::rate_limiter::RateLimiter;
use crate::service::{ArenaService, PlayerRepo};
use crate::{
    LeaderboardCaveat, LiveboardDto, LiveboardUpdate, PlayerAlias, PlayerId, SceneId, TeamName,
    YourScoreDto,
};
use std::cmp::Reverse;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

// TODO: was pub(crate)
/// Manages the live leaderboard of an arena.
pub struct LiveboardRepo<G: ArenaService> {
    pending_player_count: u32,
    pub(crate) player_count: u32,
    pending: Vec<((SceneId, PlayerId), LiveboardDto)>,
    processing: bool,
    /// The most recently computed top X leaders.
    liveboard: Arc<[LiveboardDto]>,
    pub(crate) dirty: bool,
    update_rate_limiter: RateLimiter,
    _spooky: PhantomData<G>,
}

// TODO: was pub(crate)
#[derive(Debug, Default)]
pub struct PlayerLiveboardData {
    /// Snapshot from the last liveboard computation.
    pub(crate) score: Score,
    /// Snapshot from the last liveboard computation.
    pub(crate) rank: Option<u16>,
}

#[derive(Copy, Clone, Debug, Default)]
pub enum Score {
    #[default]
    None,
    Proxy(PlayerId),
    Some(u32),
}

impl Score {
    pub fn some(self) -> Option<u32> {
        if let Self::Some(some) = self {
            Some(some)
        } else {
            None
        }
    }
}

impl<G: ArenaService> Default for LiveboardRepo<G> {
    fn default() -> Self {
        Self {
            pending_player_count: 0,
            player_count: 0,
            pending: Default::default(),
            liveboard: Vec::new().into(),
            dirty: false,
            processing: false,
            update_rate_limiter: RateLimiter::new(Duration::from_secs(1), 0),
            _spooky: PhantomData,
        }
    }
}

impl<G: ArenaService> LiveboardRepo<G> {
    pub(crate) fn process(&mut self, scene_id: SceneId, service: &G, players: &PlayerRepo<G>) {
        if !self.processing {
            return;
        }
        self.pending_player_count += players.real_players_live as u32;
        self.pending
            .extend(players.iter().filter_map(|(player_id, player)| {
                if !player.regulator.active() {
                    return None;
                }

                if !G::LIVEBOARD_BOTS && player.is_bot() {
                    return None;
                }

                // Game is reponsible for optionally witholding score if player is dead.
                let Score::Some(score) = player.liveboard.score else {
                    return None;
                };

                let (alias, team_name, authentic) = team_representation::<G>(
                    player.alias,
                    service.get_team_name(player_id),
                    player
                        .client()
                        .and_then(|c| c.nick_name())
                        .map(|n| n.as_str() == player.alias.as_str())
                        .unwrap_or(false),
                );
                Some((
                    (scene_id, player_id),
                    LiveboardDto {
                        alias,
                        score,
                        // TODO: get rid of this if in team rep mode?
                        visitor_id: player.client().and_then(|c| c.session.visitor_id),
                        authentic,
                        team_name,
                    },
                ))
            }));
    }

    /// Gets the leaders in the "current" liveboard without recalculation (or diffing).
    pub(crate) fn get(&self) -> &Arc<[LiveboardDto]> {
        &self.liveboard
    }

    /// Gets initializer for new client.
    pub(crate) fn initializer(
        &self,
        players_on_server: u32,
        players_online: u32,
        caveat: Option<LeaderboardCaveat>,
    ) -> LiveboardUpdate {
        LiveboardUpdate::Updated {
            liveboard: Arc::clone(&self.liveboard),
            your_score: None,
            players_on_shard: players_on_server,
            shard_per_scene: <G::Shard as ShardContextProvider<G>>::PER_SCENE,
            players_online,
            caveat,
            temporaries_available: false,
        }
    }

    pub(crate) fn your_score_nondestructive(
        &self,
        player_id: PlayerId,
        service: &G,
        players: &PlayerRepo<G>,
    ) -> Option<Option<YourScoreDto>> {
        if self.dirty {
            let player = &players[player_id];
            let score_player = match player.liveboard.score {
                Score::None => None,
                Score::Proxy(proxy_player_id) => {
                    if let Some(proxy_player) = players.get(proxy_player_id) {
                        Some((proxy_player_id, proxy_player))
                    } else {
                        None
                    }
                }
                Score::Some(_) => Some((player_id, player)),
            };
            let your_score = score_player.and_then(|(score_player_id, score_player)| {
                if let Score::Some(score) = score_player.liveboard.score
                    && let Some(ranking) = score_player.liveboard.rank
                {
                    let (alias, team_name, authentic) = team_representation::<G>(
                        score_player.alias,
                        service.get_team_name(score_player_id),
                        score_player
                            .client()
                            .and_then(|c| c.nick_name())
                            .map(|n| n.as_str() == player.alias.as_str())
                            .unwrap_or(false),
                    );
                    Some(YourScoreDto {
                        ranking,
                        inner: LiveboardDto {
                            alias,
                            team_name,
                            score,
                            visitor_id: score_player.client().and_then(|c| c.session.visitor_id),
                            authentic,
                        },
                    })
                } else {
                    None
                }
            });
            Some(your_score)
        } else {
            None
        }
    }

    /// Recalculates liveboard and generates a diff.
    #[allow(clippy::type_complexity)]
    pub(crate) fn update(&mut self, cohort: &mut impl LiveboardCohort<G>) {
        self.dirty = self.processing;
        self.processing = !self.update_rate_limiter.should_limit_rate();

        if self.dirty {
            cohort.visit_mut(|_, player| {
                player.liveboard = Default::default();
            });
            self.pending.sort_by_key(|(_, dto)| Reverse(dto.score));

            for (rank, (query, dto)) in self.pending.iter().enumerate() {
                let rank = rank.min(u16::MAX as usize) as u16;
                if let Some(player) = cohort.get_mut(*query) {
                    player.liveboard.score = Score::Some(dto.score);
                    player.liveboard.rank = Some(rank);
                }
            }

            let current_liveboard: Vec<_> = self
                .pending
                .drain(..)
                .take(G::LEADERBOARD_SIZE)
                .map(|(_, dto)| dto)
                .collect();

            self.liveboard = current_liveboard.into();
            self.player_count = std::mem::take(&mut self.pending_player_count);
        }
    }
}

pub fn team_representation<G: ArenaService>(
    alias: PlayerAlias,
    team_name: Option<TeamName>,
    authentic_alias: bool,
) -> (PlayerAlias, Option<TeamName>, bool) {
    (
        team_name
            .filter(|_| G::LIVEBOARD_LEADERBOARD_TEAM_REPRESENTATION)
            .map(|n| PlayerAlias::new_unsanitized(n.as_str()))
            .unwrap_or(alias),
        team_name.filter(|_| !G::LIVEBOARD_LEADERBOARD_TEAM_REPRESENTATION),
        authentic_alias && (!G::LIVEBOARD_LEADERBOARD_TEAM_REPRESENTATION || team_name.is_none()),
    )
}
