// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::ClientChatData;
use crate::actor::{ClientStatus, PlayerClientData, ServerMessage, SessionData};
use crate::bitcode::{self, *};
use crate::rate_limiter::RateLimiterState;
use crate::service::arena_service::Bot;
use crate::service::{
    ArenaService, BotRepo, ClientMetricData, Player, PlayerInner, PlayerRepo, Topology,
};
use crate::{
    ArenaId, ArenaQuery, ArenaSettingsDto, ArenaToken, ContinuousMetricAccumulator, PlasmaRequest,
    PlasmaRequestV1, PlasmaUpdate, PlasmaUpdateV1, PlayerId, QuestEvent, ReconnectionToken,
    ScopeClaimKey, ServerId,
};
use actix::Recipient;
use kodiak_common::rand::random;
use log::info;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;

/// Things that go along with every instance of a [`ArenaService`].
pub struct ArenaContext<G: ArenaService> {
    pub token: ArenaToken,
    pub players: PlayerRepo<G>,
    pub(crate) bots: BotRepo<G>,
    /// Other servers of the same kind (intended, but not currently guaranteed
    /// to have the same client hash).
    pub topology: Topology,
    /// Last time plasma sanctioned the existence of this arena.
    pub last_sanctioned: Instant,
    pub(crate) prune_rate_limit: RateLimiterState,
    pub(crate) prune_warn_rate_limit: RateLimiterState,
    pub(crate) send_to_plasma: SendPlasmaRequest,
    pub settings: ArenaSettingsDto<G::ArenaSettings>,
    pub tick_duration: ContinuousMetricAccumulator,
}

#[derive(Clone)]
pub struct SendPlasmaRequest {
    pub(crate) web_socket: Option<Sender<PlasmaRequest>>,
    pub(crate) local: Recipient<PlasmaUpdate>,
    pub(crate) local_server_id: ServerId,
}

impl SendPlasmaRequest {
    pub fn send(&self, request: PlasmaRequest) {
        match request {
            PlasmaRequest::V1(PlasmaRequestV1::SendServerMessage {
                recipients,
                message,
            }) if recipients.len() == 1
                && *recipients.iter().next().unwrap() == self.local_server_id =>
            {
                info!("sent {message:?} efficiently");
                let _ = self.local.do_send(PlasmaUpdate::V1(
                    vec![PlasmaUpdateV1::Parley {
                        sender: self.local_server_id,
                        message,
                    }]
                    .into(),
                ));
            }
            request => {
                if let Some(websocket) = self.web_socket.as_ref() {
                    let _ = websocket.try_send(request);
                }
            }
        }
    }
}

#[derive(Debug, Encode, Decode)]
pub struct RedirectedPlayer {
    old_player_id: PlayerId,
    old_token: ReconnectionToken,
    ip_address: IpAddr,
    /// Ensure player stays signed.
    session: SessionData,
    chat: ClientChatData,
    metrics: ClientMetricData,
}

impl<G: ArenaService> ArenaContext<G> {
    pub(crate) fn new(
        server_id: ServerId,
        arena_id: ArenaId,
        send_to_plasma: SendPlasmaRequest,
    ) -> Self {
        ArenaContext {
            token: ArenaToken(random()),
            bots: Default::default(),
            players: Default::default(),
            topology: Topology::new(server_id, arena_id),
            send_to_plasma,
            // If was sanctioned in last few minutes, act like it still is, just in cast another
            // server doesn't receive a new topology and tries to transfer players.
            last_sanctioned: Instant::now(),
            prune_rate_limit: Default::default(),
            prune_warn_rate_limit: Default::default(),
            settings: Default::default(),
            tick_duration: ContinuousMetricAccumulator::default(),
        }
    }

    pub fn set_settings(&mut self, settings: ArenaSettingsDto<G::ArenaSettings>) {
        self.settings = settings;

        self.settings.bot_aggression = self
            .settings
            .bot_aggression
            .filter(|a| !a.is_nan())
            .map(|a| a.clamp(0.0, 10.0));

        let hard_max = if cfg!(debug_assertions) { 64 } else { 1024 };
        if let Some(bots) = &mut self.settings.bots {
            *bots = if self.topology.local_arena_id.realm_id.is_public_default() {
                *bots
            } else {
                (*bots)
                    .max(G::GAME_CONSTANTS.min_temporary_server_bots)
                    .min(G::GAME_CONSTANTS.max_temporary_server_bots)
            }
            .min(hard_max);
        }
    }

    /// Game-specific victory.
    pub fn tally_victory(&mut self, victor: PlayerId, defeated: PlayerId) {
        let Some((victor, defeated)) = self.players.get_two_mut(victor, defeated) else {
            return;
        };
        let Some(victor_client) = victor.inner.client_mut() else {
            return;
        };
        let defeated_score = defeated.liveboard.score.some().unwrap_or_default();
        victor_client.push_quest(QuestEvent::Victory {
            bot: defeated.is_bot(),
            score: defeated_score,
        });

        if !self.topology.local_arena_id.realm_id.is_public_default() || defeated.is_bot() {
            return;
        }
        let value = victor_client
            .claim(ScopeClaimKey::victories())
            .map(|c| c.value)
            .unwrap_or_default()
            .saturating_add(1);
        victor_client.update_claim(ScopeClaimKey::victories(), value, None);

        let victor_score = victor.liveboard.score.some().unwrap_or_default();
        let inc = if defeated_score / 2 > victor_score
            && defeated.was_alive_timestamp.elapsed() > Duration::from_secs(60)
        {
            Some(ScopeClaimKey::superior_victories())
        } else if defeated_score < victor_score / 2
            && victor.was_alive_timestamp.elapsed() > Duration::from_secs(60)
        {
            Some(ScopeClaimKey::inferior_victories())
        } else {
            None
        };
        if let Some(inc) = inc {
            let value = victor_client
                .claim(inc)
                .map(|c| c.value)
                .unwrap_or_default()
                .saturating_add(1);
            victor_client.update_claim(inc, value, None);
        }
    }

    /// Sends player at `player_id` to server at `server_id`.
    ///
    /// Game should remove and forget player as if `player_quit`
    /// and `player_left` were both called.
    ///
    /// **Panics**
    ///
    /// - If player at `player_id` does not exist or is a bot.
    /// - If the client is not connected.
    pub fn send_player(
        &mut self,
        player_id: PlayerId,
        server_id: ServerId,
        arena_id: ArenaId,
    ) -> RedirectedPlayer {
        let player = &mut self.players[player_id];
        if !player.regulator.active() {
            panic!("cannot send inactive player");
        }
        player.regulator.leave_now();
        self.send_player_impl(
            player_id,
            server_id,
            ArenaQuery::Specific(arena_id, None),
            true,
        )
    }

    /// Sends player at `player_id` to server at `server_id`.
    ///
    /// Game should remove and forget player as if `player_quit`
    /// and arrange for `player_left` to be called.
    ///
    /// Engine should ensure play is stopped, if applicable, for metrics purposes.
    ///
    /// **Panics**
    ///
    /// - If player at `player_id` does not exist or is a bot.
    /// - If the client is not connected.
    pub(crate) fn send_player_impl(
        &mut self,
        player_id: PlayerId,
        server_id: ServerId,
        arena_id: ArenaQuery,
        game: bool,
    ) -> RedirectedPlayer {
        let player = &mut self.players[player_id];
        let client = player.client_mut().expect("TODO send bots");

        let (observer, active) = match std::mem::replace(
            &mut client.status,
            ClientStatus::Pending {
                expiry: Instant::now(),
            },
        ) {
            ClientStatus::Pending { .. } => {
                panic!("sending pending player");
            }
            ClientStatus::Connected {
                observer, active, ..
            } => (Some(observer), active),
            ClientStatus::Limbo { .. } => (None, None),
            ClientStatus::Redirected { .. } => {
                panic!("player already redirected");
            }
            ClientStatus::LeavingLimbo { .. } => {
                panic!("sending player leaving limbo");
            }
        };

        client.status = ClientStatus::Redirected {
            expiry: Instant::now() + G::LIMBO.max(Duration::from_secs(1)),
            server_id,
            id_token: None,
            observer,
            active,
            send_close: true,
        };

        client.push_quest(QuestEvent::Arena {
            server_id,
            arena_id,
            game,
        });

        if arena_id
            .specific()
            .map(|s| s.realm_id != self.topology.local_arena_id.realm_id)
            .unwrap_or(true)
        {
            client.chat.inbox = Default::default();
            client.chat.join_announced = None;
        }

        RedirectedPlayer {
            old_player_id: player_id,
            old_token: client.token,
            chat: client.chat.clone(),
            metrics: client.metrics.clone(),
            session: client.session.clone(),
            ip_address: client.ip_address,
        }
    }

    /// Reconstitutes a player and returns the [`PlayerId`] it got assigned. Game should act like
    /// `player_joined` was called.
    ///
    /// **Panics**
    ///
    /// If server runs out of [`PlayerId`]s.
    pub fn receive_player(
        &mut self,
        server_id: ServerId,
        arena_id: ArenaId,
        redirected_player: RedirectedPlayer,
    ) -> PlayerId {
        let mut i = 0;
        let player_id = loop {
            let player_id = PlayerId::nth_client(i).expect("ran out of PlayerIds");
            if !self.players.contains(player_id) {
                break player_id;
            }
            i += 1;
        };

        info!(
            "{} receiving {player_id:?}: {redirected_player:?}",
            self.topology.local_arena_id
        );

        let was_alive = redirected_player.metrics.play_started.is_some()
            && redirected_player.metrics.play_stopped.is_none();
        let mut client = PlayerClientData::new(
            redirected_player.chat,
            redirected_player.metrics,
            redirected_player.ip_address,
        );
        client.session = redirected_player.session;
        client.status = ClientStatus::Limbo {
            expiry: Instant::now() + Duration::from_secs(10),
        };

        self.send_to_plasma
            .send(PlasmaRequest::V1(PlasmaRequestV1::SendServerMessage {
                recipients: std::iter::once(server_id).collect(),
                message: serde_json::to_value(ServerMessage::Ack {
                    old_arena_id: arena_id,
                    old_player_id: redirected_player.old_player_id,
                    old_token: redirected_player.old_token,
                    arena_id: self.topology.local_arena_id,
                    player_id,
                    token: client.token,
                })
                .unwrap(),
            }));

        let mut player = Player::new(PlayerInner::Client(client));
        assert!(player.regulator.join());
        player.was_alive = was_alive;
        // TODO was_ever_alive.
        player.was_ever_alive = player.was_alive;
        self.players.insert(player_id, player);

        player_id
    }

    /// Sends `message` to server at `server_id`. The message may or may not arrive but we won't find out.
    pub fn send_server_message(
        &mut self,
        server_id: ServerId,
        arena_id: ArenaId,
        message: serde_json::Value,
    ) {
        self.send_to_plasma
            .send(PlasmaRequest::V1(PlasmaRequestV1::SendServerMessage {
                recipients: std::iter::once(server_id).collect(),
                message: serde_json::to_value(ServerMessage::Game {
                    sender_arena_id: self.topology.local_arena_id,
                    arena_id,
                    message,
                })
                .unwrap(),
            }));
    }

    pub fn min_players(&self) -> usize {
        self.settings
            .bots
            .map(|n| n as usize)
            .unwrap_or(G::Bot::AUTO.min_bots)
    }
}
