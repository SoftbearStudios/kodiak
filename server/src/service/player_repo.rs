// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::PlayerClientData;
use crate::rate_limiter::RateLimiterState;
use crate::service::{
    ArenaService, InvitationRepo, MetricRepo, PlayerBotData, PlayerLeaderboardData,
    PlayerLiveboardData, Regulator, Score,
};
use crate::util::diff_large_n;
use crate::{
    ArenaId, ArenaMap, ChatMessage, InvitationDto, MessageDto, PlayerAlias, PlayerDto, PlayerId,
    PlayerUpdate, RankNumber, ScopeClaimKey, ServerId, TeamId,
};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Players of an arena.
pub struct PlayerRepo<G: ArenaService> {
    /// Ground-truth player data.
    players: ArenaMap<PlayerId, Player<G>>,
    /// Previous DTO's sent to clients.
    previous: Arc<[PlayerDto]>,
    /// Recently computed cache of number of real players (not bots).
    pub(crate) real_players: u16,
    /// Recently computed cache of number of real players (not bots) that were alive recently.
    pub real_players_live: u16,
    pub(crate) claim_update_rate_limit: RateLimiterState,
}

impl<G: ArenaService> Default for PlayerRepo<G> {
    fn default() -> Self {
        Self {
            players: Default::default(),
            real_players: 0,
            real_players_live: 0,
            previous: Vec::new().into(),
            claim_update_rate_limit: Default::default(),
        }
    }
}

impl<G: ArenaService> Deref for PlayerRepo<G> {
    type Target = ArenaMap<PlayerId, Player<G>>;

    fn deref(&self) -> &Self::Target {
        &self.players
    }
}

impl<G: ArenaService> DerefMut for PlayerRepo<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.players
    }
}

impl<G: ArenaService> PlayerRepo<G> {
    /// Removes a player, performing mandatory cleanup steps.
    pub(crate) fn forget(
        &mut self,
        player_id: PlayerId,
        invitations: &mut InvitationRepo<G>,
    ) -> Player<G> {
        let mut player = self
            .players
            .remove(player_id)
            .expect("forgetting non-existing player");
        invitations.forget_player_invitation(&mut player);
        player
    }

    /// Updates cache of whether players are alive, tallying metrics in the process.
    ///
    /// Returns join announcements.
    pub(crate) fn update_is_alive_and_team_id(
        &mut self,
        service: &mut G,
        metrics: &mut MetricRepo<G>,
        server_id: ServerId,
        arena_id: ArenaId,
    ) -> Vec<Arc<MessageDto>> {
        let mut announcements = Vec::new();
        for (player_id, p) in self.iter_mut() {
            if let Some(add) = p.regulator.tick() {
                if add {
                    service.player_joined(player_id, p);
                } else {
                    service.player_left(player_id, p);
                }
            }

            // Whether joined game and not yet left. Important not to check liveness/teamid when not ingame.
            let ingame = p.regulator.active();

            let score = if ingame {
                p.alias = service.get_alias(player_id);
                service.get_score(player_id)
            } else {
                Score::None
            };
            p.leaderboard.update_score(score);
            p.liveboard.score = score;

            if let Some(client) = p.client_mut() {
                client.with_quest(|quest| {
                    if let Score::Some(score) = score {
                        quest.update_score(score);
                    }
                });
            }

            let is_alive = ingame && service.is_alive(player_id);

            if is_alive != p.was_alive {
                if is_alive {
                    // Play started.
                    metrics.start_play(p);

                    let rank = p.rank();
                    if rank >= Some(RankNumber::Rank4)
                        && let Some(client) = p.inner.client_mut()
                        && client
                            .claim(ScopeClaimKey::announcement_preference())
                            .map(|v| v.value != 0)
                            .unwrap_or(true)
                        && let Some(visitor_id) = client.session.visitor_id
                        && client.chat.join_announced != Some(arena_id.scene_id)
                    {
                        client.chat.join_announced = Some(arena_id.scene_id);
                        announcements.push(Arc::new(MessageDto {
                            alias: PlayerAlias::authority(),
                            visitor_id: None,
                            team_name: None,
                            message: ChatMessage::Join {
                                alias: p.alias,
                                visitor_id: Some(visitor_id),
                                authentic: client
                                    .nick_name()
                                    .map(|n| n.as_str() == p.alias.as_str())
                                    .unwrap_or(false),
                                rank,
                                arena_id,
                                server_number: server_id.number,
                            },
                            authority: true,
                            authentic: false,
                            whisper: false,
                        }));
                    }
                } else {
                    // Play stopped.
                    metrics.stop_play(p);
                }

                p.was_alive = is_alive;
                p.was_ever_alive = true;
                p.was_alive_timestamp = Instant::now();
            }
            p.was_out_of_game = p.is_out_of_game();
            p.team_id = if ingame {
                service.get_team_id(player_id)
            } else {
                None
            };

            // TODO: team quest event.
        }

        announcements
    }

    /// Computes current set of player dtos, and number of real players (total and live).
    fn compute(&self) -> (Vec<PlayerDto>, u16, u16) {
        let mut real_players = 0;
        let mut real_players_live = 0;

        let player_dtos = self
            .iter()
            .filter_map(|(player_id, p)| {
                if !p.is_bot() {
                    real_players += 1;
                }

                if !p.regulator.active() {
                    None
                } else {
                    if !p.is_bot() && !p.is_out_of_game() {
                        real_players_live += 1;
                    }

                    let (_visitor_id, authentic, admin, moderator) = p
                        .client()
                        .map(|c| {
                            (
                                c.session.visitor_id,
                                c.nick_name()
                                    .map(|n| n.as_str() == p.alias.as_str())
                                    .unwrap_or(false),
                                c.admin(),
                                c.moderator(),
                            )
                        })
                        .unwrap_or((None, false, false, false));

                    Some(PlayerDto {
                        alias: p.alias,
                        admin,
                        moderator,
                        player_id,
                        team_id: p.team_id,
                        //visitor_id,
                        //user_id,
                        authentic,
                    })
                }
            })
            .collect();

        (player_dtos, real_players, real_players_live)
    }

    /// Gets initializer for new client.
    pub(crate) fn initializer(&self) -> PlayerUpdate {
        PlayerUpdate::Updated {
            added: Arc::clone(&self.previous),
            removed: Vec::new().into(),
        }
    }

    /// Computes a diff, and updates cached dtos.
    #[allow(clippy::type_complexity)]
    pub(crate) fn delta(&mut self) -> Option<(Arc<[PlayerDto]>, Arc<[PlayerId]>)> {
        let (current_players, real_players, real_players_live) = self.compute();

        self.real_players = real_players;
        self.real_players_live = real_players_live;

        if let Some((added, removed)) =
            diff_large_n(&self.previous, &current_players, |dto| dto.player_id)
        {
            self.previous = current_players.into();
            Some((added.into(), removed.into()))
        } else {
            None
        }
    }

    pub fn iter_player_ids(&self) -> impl Iterator<Item = PlayerId> + '_ {
        self.keys()
    }
}

#[derive(Debug)]
pub enum PlayerInner<G: ArenaService> {
    Client(PlayerClientData<G>),
    Bot(PlayerBotData<G>),
}

impl<G: ArenaService> PlayerInner<G> {
    pub fn is_client(&self) -> bool {
        matches!(self, Self::Client(_))
    }

    pub fn client(&self) -> Option<&PlayerClientData<G>> {
        if let Self::Client(client) = self {
            Some(client)
        } else {
            None
        }
    }

    pub fn client_mut(&mut self) -> Option<&mut PlayerClientData<G>> {
        if let Self::Client(client) = self {
            Some(client)
        } else {
            None
        }
    }

    pub fn is_bot(&self) -> bool {
        matches!(self, Self::Bot(_))
    }

    pub fn bot(&self) -> Option<&PlayerBotData<G>> {
        if let Self::Bot(bot) = self {
            Some(bot)
        } else {
            None
        }
    }

    pub fn bot_mut(&mut self) -> Option<&mut PlayerBotData<G>> {
        if let Self::Bot(bot) = self {
            Some(bot)
        } else {
            None
        }
    }
}

/// Data stored per real or bot player.
#[derive(Debug)]
pub struct Player<G: ArenaService> {
    /// Regulator state (prevent join too fast after leave).
    pub(crate) regulator: Regulator,
    /// Current player alias.
    pub(crate) alias: PlayerAlias,
    /// Current player score and rank.
    pub(crate) liveboard: PlayerLiveboardData,
    /// Highscore.
    pub(crate) leaderboard: PlayerLeaderboardData,
    /// Last team id.
    pub(crate) team_id: Option<TeamId>,
    /// Whether the player was alive last time we checked.
    pub(crate) was_alive: bool,
    /// Whether the player was out of game last time we checked.
    pub(crate) was_out_of_game: bool,
    /// Whether the player was *ever* alive.
    pub(crate) was_ever_alive: bool,
    /// When was_alive was set to its current value.
    pub(crate) was_alive_timestamp: Instant,
    pub inner: PlayerInner<G>,
}

impl<G: ArenaService> Player<G> {
    pub fn new(inner: PlayerInner<G>) -> Self {
        Self {
            regulator: Default::default(),
            alias: Default::default(),
            was_alive: false,
            was_out_of_game: false,
            was_ever_alive: false,
            was_alive_timestamp: Instant::now(),
            inner,
            team_id: None,
            liveboard: Default::default(),
            leaderboard: Default::default(),
        }
    }

    /// If player is a real player, returns their client data.
    pub fn client(&self) -> Option<&PlayerClientData<G>> {
        self.inner.client()
    }

    /// If player is a real player, returns a mutable reference to their client data.
    pub fn client_mut(&mut self) -> Option<&mut PlayerClientData<G>> {
        self.inner.client_mut()
    }

    /// Use `client_data()? -> Option<&G::ClientData>` where None means bot.
    pub fn client_data(&self) -> Option<Option<&G::ClientData>> {
        if let Some(client) = self.client() {
            if let Some(data) = client.data() {
                Some(Some(data))
            } else {
                None
            }
        } else {
            Some(None)
        }
    }

    /// Use `client_data_mut()? -> Option<&mut G::ClientData>` where None means bot.
    ///
    /// This prioritizes not dropping client updates, but client and server may not
    /// be in sync (client may send message to old server and deliver it to new server).
    pub fn client_data_mut(&mut self) -> Option<Option<&mut G::ClientData>> {
        if let Some(client) = self.client_mut() {
            if let Some(data) = client.data_mut() {
                Some(Some(data))
            } else {
                None
            }
        } else {
            Some(None)
        }
    }

    /// Use `fenced_client_data()? -> Option<&G::ClientData>` where None means bot.
    ///
    /// See `fenced_client_data_mut` comment.
    pub fn fenced_client_data(&self) -> Option<Option<&G::ClientData>> {
        if let Some(client) = self.client() {
            if let Some(data) = client.fenced_data() {
                Some(Some(data))
            } else {
                None
            }
        } else {
            Some(None)
        }
    }

    /// Use `fenced_client_data_mut()? -> Option<&mut G::ClientData>` where None means bot.
    ///
    /// This is for when the client and server must be in sync (e.g. lockstep) but its ok
    /// to drop client updates.
    pub fn fenced_client_data_mut(&mut self) -> Option<Option<&mut G::ClientData>> {
        if let Some(client) = self.client_mut() {
            if let Some(data) = client.fenced_data_mut() {
                Some(Some(data))
            } else {
                None
            }
        } else {
            Some(None)
        }
    }

    pub fn rank(&self) -> Option<RankNumber> {
        self.client()
            .and_then(|c| c.session.claim(ScopeClaimKey::rank()))
            .and_then(|n| RankNumber::new(n.value.min(u8::MAX as u64) as u8))
    }

    /// Gets any invitation accepted by the player (always [`None`] for bots).
    pub fn invitation_accepted(&self) -> Option<&InvitationDto> {
        self.client()
            .and_then(|c| c.invitation.invitation_accepted.as_ref())
    }

    /// A lagging indicator of [`ArenaService::is_alive`], updated after the game runs.
    pub(crate) fn is_alive(&self) -> bool {
        self.was_alive
    }

    /// If the player was recently alive, this returns how long they were alive for.
    #[allow(unused)]
    pub(crate) fn alive_duration(&self) -> Option<Duration> {
        self.was_alive.then(|| self.was_alive_timestamp.elapsed())
    }

    /// If the player was recently not alive, this returns how long they were not alive for.
    ///
    pub(crate) fn not_alive_duration(&self) -> Option<Duration> {
        (!self.was_alive).then(|| self.was_alive_timestamp.elapsed())
    }

    /// Returns true iff player is a bot (their id is a bot id).
    pub fn is_bot(&self) -> bool {
        self.inner.is_bot()
    }

    /// Returns true iff the player 1) never played yet 2) stopped playing over half a minute ago.
    pub fn is_out_of_game(&self) -> bool {
        !self.was_ever_alive
            || self.not_alive_duration().unwrap_or(Duration::ZERO) > Duration::from_secs(30)
    }

    /// Is a client and is connected. TODO remove this when any player can be sent.
    pub fn is_connected(&self) -> bool {
        self.client().map_or(false, |c| c.status.is_connected())
    }
}
