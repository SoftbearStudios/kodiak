// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::system_actlet::ServerPickerItem;
use super::{ClientActlet, Health};
use crate::actor::{ClientStatus, ServerActor};
use crate::net::{load_domains, WebSocket};
use crate::observer::ObserverUpdate;
use crate::service::{ArenaService, MessageAttribution, MetricRepo, RealmRepo, SendPlasmaRequest};
use crate::{
    decode_buffer, ActiveHeartbeat, ArenaHeartbeat, ArenaId, ArenaQuery, ChatRecipient, ClientHash,
    ClientUpdate, CommonUpdate, DomainDto, DomainName, GameId, InstancePickerDto, MessageDto,
    NonZeroUnixMillis, PlasmaRequest, PlasmaRequestV1, PlasmaUpdate, PlasmaUpdateV1, PlayerId,
    RealmAcl, RealmHeartbeat, RealmId, RealmUseTopology, ReconnectionToken, RedirectedPlayer,
    RegionId, SceneId, SceneUseTopology, ServerId, ServerRole, ServerToken, ServerUseTopology,
    TranslationsFile, WebsocketConnectQuery,
};
use actix::dev::ContextFutureSpawner;
use actix::{Actor, ActorFutureExt, AsyncContext, Handler, Recipient, WrapFuture};
use axum_server::tls_rustls::RustlsConfig;
use bytes::Bytes;
use kodiak_common::rand::{thread_rng, Rng};
use log::{error, info, warn};
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct PlasmaActlet {
    infrastructure: Option<Recipient<PlasmaUpdate>>,
    /// Last outbound heartbeat time.
    last_heartbeat: Option<Instant>,
    last_acknowledged_heartbeat: Option<Instant>,
    /// Last server log update.
    last_server_log: Option<Instant>,
    last_quest_samples: Option<Instant>,
    redirect_server_number: &'static AtomicU8,
    pub role: ServerRole,
    pub redirecting_since: Option<Instant>,
    pub server_token: &'static AtomicU64,
    pub(crate) rustls_config: RustlsConfig,
    pub(crate) cors_alternative_domains: &'static Mutex<Arc<[DomainName]>>,
    pub(crate) date_certificate_expires: Option<NonZeroUnixMillis>,
    pub servers: HashMap<ServerId, ServerUseTopology>,
    pub(crate) web_socket: WebSocket,
    pub(crate) health: Health,
    pub(crate) quest_fraction: f32,
    domain_backup: Option<Arc<str>>,
    last_hiccup: Option<Instant>,
    file_client: reqwest::Client,
}

impl PlasmaActlet {
    pub(crate) fn new<G: ArenaService>(
        redirect_server_number: &'static AtomicU8,
        server_token: &'static AtomicU64,
        rustls_config: RustlsConfig,
        cors_alternative_domains: &'static Mutex<Arc<[DomainName]>>,
        domain_backup: Option<Arc<str>>,
    ) -> Self {
        let mut date_certificate_expires = None;
        if let Some(domain_backup) = &domain_backup {
            if let Ok(contents) = std::fs::read_to_string(&**domain_backup) {
                if let Ok(domains) = serde_json::from_str::<Box<[DomainDto]>>(&contents) {
                    if let Some((config, date)) = load_domains::<G>(&domains) {
                        rustls_config.reload_from_config(config);
                        date_certificate_expires = Some(date);
                        warn!("successfully read domain backup");
                    }
                }
            }
        }

        Self {
            redirect_server_number,
            server_token,
            rustls_config,
            cors_alternative_domains,
            date_certificate_expires,
            domain_backup,
            role: ServerRole::Unlisted,
            redirecting_since: None,
            infrastructure: None,
            last_heartbeat: None,
            last_acknowledged_heartbeat: None,
            last_server_log: None,
            last_quest_samples: None,
            quest_fraction: if cfg!(debug_assertions) { 1.0 } else { 0.2 },
            web_socket: WebSocket::new(),
            servers: Default::default(),
            health: Default::default(),
            last_hiccup: None,
            file_client: reqwest::ClientBuilder::new()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .read_timeout(Duration::from_secs(15))
                .user_agent("game_server")
                .tcp_nodelay(true)
                .tcp_keepalive(Some(Duration::from_secs(10)))
                .build()
                .unwrap(),
        }
    }

    pub(crate) fn set_infrastructure<G: ArenaService>(
        &mut self,
        game_id: GameId,
        server_id: ServerId,
        infrastructure: Recipient<PlasmaUpdate>,
        realms: &mut RealmRepo<G>,
    ) {
        self.infrastructure = Some(infrastructure.clone());
        self.flush_arenas(server_id, infrastructure.clone(), realms);
        self.web_socket.spawn(
            {
                let query = WebsocketConnectQuery {
                    game_id,
                    server_id,
                    server_token: ServerToken(
                        NonZeroU64::new(self.server_token.load(Ordering::Relaxed)).unwrap(),
                    ),
                };
                let query_string = serde_urlencoded::to_string(query).unwrap();
                format!("wss://softbear.com/ws/?{query_string}")
            },
            infrastructure,
        );
    }

    /// Reports whether a particular arena is provisioned on a particular server.
    pub(crate) fn is_sanctioned(&self, server_id: ServerId, arena_id: ArenaId) -> bool {
        // No longer needed, Plasma is good enough with local server topologies.
        /*
        if server_id.kind.is_local() {
            // For testing.
            return true;
        }
        */
        if arena_id.realm_id.is_public_default()
            && arena_id.scene_id == Default::default()
            && !self.role.is_realms()
        {
            // Fail-safe to keep the public server online if `servers` is deficient.
            return true;
        }

        self.servers
            .get(&server_id)
            .and_then(|server| {
                server
                    .realm(arena_id.realm_id)
                    .filter(|r| r.scenes.contains_key(&arena_id.scene_id))
            })
            .is_some()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update<G: ArenaService>(
        &mut self,
        server_id: ServerId,
        realms: &mut RealmRepo<G>,
        clients: &mut ClientActlet<G>,
        metrics: &mut MetricRepo<G>,
        region_id: RegionId,
        client_hash: ClientHash,
        ctx: &mut <ServerActor<G> as Actor>::Context,
    ) {
        let cpu = self.health.cpu() + self.health.cpu_steal();
        let ram = self.health.ram();
        let missed_ticks = self.health.missed_ticks();
        let now = Instant::now();

        let since_heartbeat = self
            .last_heartbeat
            .map(|last_poll| now.saturating_duration_since(last_poll))
            .unwrap_or(Duration::from_secs(3600));
        let since_message = self
            .last_acknowledged_heartbeat
            .map(|last_poll| now.saturating_duration_since(last_poll))
            .unwrap_or(Duration::from_secs(3600));
        let hiccup = since_message > Duration::from_secs(67);
        if hiccup {
            if self.last_hiccup.is_none() {
                warn!("detected network hiccup");
            }
            self.last_hiccup = Some(now);
        }
        let since_hiccup = self
            .last_hiccup
            .map(|last_poll| now.saturating_duration_since(last_poll))
            .unwrap_or(Duration::from_secs(3600));
        let recent_hiccup = since_hiccup < Duration::from_secs(180);
        if !recent_hiccup {
            self.last_hiccup = None;
        }

        if since_heartbeat
            < Duration::from_secs(if hiccup {
                15
            } else if recent_hiccup {
                30
            } else if matches!(region_id, RegionId::Asia | RegionId::SouthAmerica) {
                // Mitigate frequent disconnects.
                45
            } else {
                60
            })
        {
            return;
        }
        self.last_heartbeat = Some(now);
        let mut requests = Vec::new();
        if since_message > Duration::from_secs(130) {
            // Fail-open to avoid locking players out.
            self.set_role(ServerRole::Unlisted);
            self.servers = HashMap::new();
            let player_count = realms
                .main()
                .map(|a| a.arena.arena_context.players.real_players_live as u16)
                .unwrap_or_default();
            self.servers.insert(
                server_id,
                ServerUseTopology {
                    datacenter: "?".to_owned(),
                    default_realm: Some(RealmUseTopology {
                        acl: RealmAcl::default(),
                        scenes: {
                            let mut ret = HashMap::new();
                            ret.insert(
                                SceneId::default(),
                                SceneUseTopology {
                                    player_count,
                                    settings: None,
                                },
                            );
                            ret
                        },
                    }),
                    other_realms: Default::default(),
                    region_id,
                },
            );
            self.flush_arenas(server_id, ctx.address().recipient(), realms);

            requests.push(PlasmaRequestV1::RegisterServer {
                date_started: Some(metrics.startup),
            });
        }
        let date_synchronized = NonZeroUnixMillis::now();
        requests.push(PlasmaRequestV1::Heartbeat {
            cpu,
            ram,
            missed_ticks,
            client_hash,
            date_certificate_expires: self.date_certificate_expires,
            realms: realms
                .realms()
                .map(|(realm_id, r)| {
                    (
                        realm_id,
                        RealmHeartbeat {
                            scenes: r
                                .scene_repo
                                .iter()
                                .map(|(scene_id, a)| {
                                    (
                                        scene_id,
                                        ArenaHeartbeat {
                                            player_count: a
                                                .arena
                                                .arena_context
                                                .players
                                                .real_players_live
                                                as u16,
                                            tick_duration: a
                                                .arena
                                                .arena_context
                                                .tick_duration
                                                .average(),
                                            actives: a
                                                .arena
                                                .arena_context
                                                .players
                                                .iter()
                                                .filter_map(|(player_id, p)| {
                                                    let client = p.client()?;
                                                    if !client.session.active_heartbeat {
                                                        return None;
                                                    }
                                                    Some((
                                                        player_id,
                                                        ActiveHeartbeat {
                                                            visitor_id: Some(
                                                                client.session.visitor_id?,
                                                            ),
                                                        },
                                                    ))
                                                })
                                                .collect(),
                                            /*
                                            settings: Some(
                                                serde_json::to_value(
                                                    &a.arena.arena_context.settings,
                                                )
                                                .unwrap(),
                                            ),
                                            */
                                            settings: None,
                                        },
                                    )
                                })
                                .collect(),
                        },
                    )
                })
                .collect(),
            claims: clients
                .trailing_claims
                .drain(..)
                .chain(
                    realms
                        .iter_mut()
                        .flat_map(|(arena_id, scene)| {
                            scene
                                .arena
                                .arena_context
                                .players
                                .iter_mut()
                                .map(move |(player_id, player)| (arena_id, player_id, player))
                        })
                        .filter_map(|(arena_id, player_id, player)| {
                            player.client_mut().and_then(move |c| {
                                c.claim_update(date_synchronized, arena_id, player_id)
                            })
                        }),
                )
                .collect(),
        });
        let closing = self.role.is_closing();
        if closing || !recent_hiccup {
            let max_flush_delay = if closing {
                Duration::from_secs(110)
            } else {
                Duration::from_secs(3600)
            };
            // Very dangerous: Do not log anything while this is held.
            let mut logs = crate::cli::LOGS.lock().unwrap();
            if logs.len() >= 512
                || (!logs.is_empty()
                    && self
                        .last_server_log
                        .map(|t| t.elapsed() > max_flush_delay)
                        .unwrap_or(true))
            {
                let server_log = std::mem::take(&mut *logs);
                drop(logs);
                info!("uploading {} server logs", server_log.len());
                requests.push(PlasmaRequestV1::UpdateServerLog {
                    server_log: server_log.into(),
                });
                self.last_server_log = Some(now);
            } else {
                drop(logs);
            }
            let quests = &mut metrics.pending_quests;
            if quests.len() >= 20
                || (!quests.is_empty()
                    && self
                        .last_quest_samples
                        .map(|t| t.elapsed() > max_flush_delay)
                        .unwrap_or(true))
            {
                warn!("uploading {} quest sample(s)", quests.len());
                requests.push(PlasmaRequestV1::UpdateQuestSamples {
                    quest_samples: std::mem::take(quests).into(),
                });
                self.last_quest_samples = Some(now);
            }
        }
        for request in requests {
            self.do_request(request);
        }
        if matches!(region_id, RegionId::Asia | RegionId::SouthAmerica) {
            warn!("sending heartbeat");
        }
    }

    pub(crate) fn do_request(&self, request: PlasmaRequestV1) {
        self.web_socket.do_send(PlasmaRequest::V1(request));
    }

    fn set_role(&mut self, role: ServerRole) {
        self.role = role;
        if self.redirecting_since.is_some() != role.is_redirected() {
            self.redirecting_since = if role.is_redirected() {
                Some(Instant::now())
            } else {
                None
            }
        }
        self.redirect_server_number.store(
            role.redirect().map(|s| s.0.get()).unwrap_or(0),
            Ordering::Relaxed,
        );
    }

    fn flush_arenas<G: ArenaService>(
        &mut self,
        server_id: ServerId,
        recipient: Recipient<PlasmaUpdate>,
        realms: &mut RealmRepo<G>,
    ) {
        let send_plasma_request = SendPlasmaRequest {
            web_socket: self.web_socket.sender.clone(),
            local: recipient.clone(),
            local_server_id: server_id,
        };
        if let Some(server) = self.servers.get(&server_id) {
            for (realm_id, realm) in server.realms() {
                for (scene_id, instance) in &realm.scenes {
                    let (_, scene) = realms.get_mut_or_default(
                        server_id,
                        ArenaId::new(realm_id, *scene_id),
                        send_plasma_request.clone(),
                    );
                    if let Some(settings) = &instance.settings {
                        match serde_json::from_value(settings.clone()) {
                            Ok(settings) => {
                                scene.arena.arena_context.set_settings(settings);
                            }
                            Err(e) => {
                                error!("failed to deserialize settings {e} {settings:?}");
                            }
                        }
                    }
                }
            }
        }
        // Fail-safe, expedite.
        if !self.role.is_realms() {
            realms.get_mut_or_default(server_id, ArenaId::default(), send_plasma_request);
        }
    }
}

impl<G: ArenaService> Handler<PlasmaUpdate> for ServerActor<G> {
    type Result = ();

    fn handle(&mut self, response: PlasmaUpdate, ctx: &mut Self::Context) -> Self::Result {
        // println!("received plasma update {response:?}");

        #[allow(clippy::infallible_destructuring_match)]
        let updates = match response {
            PlasmaUpdate::V1(updates) => updates,
        };

        for update in Vec::from(updates) {
            match update {
                PlasmaUpdateV1::Heartbeat {} => {
                    self.plasma.last_acknowledged_heartbeat = Some(Instant::now());
                }
                PlasmaUpdateV1::Claims { claims } => {
                    for dto in Vec::from(claims) {
                        let Some(scene) = self.realms.get_mut(dto.arena_id) else {
                            continue;
                        };
                        let Some(player) = scene.arena.arena_context.players.get_mut(dto.player_id)
                        else {
                            continue;
                        };
                        let Some(client) = player.client_mut() else {
                            continue;
                        };
                        if client.session.visitor_id != Some(dto.visitor_id) {
                            continue;
                        }
                        client.session.claims.date_synchronized = client
                            .session
                            .claims
                            .date_synchronized
                            .max(dto.claims.date_synchronized);
                        let now = NonZeroUnixMillis::now();
                        for (key, claim) in dto.claims.claims {
                            match client.session.claims.claims.entry(key) {
                                Entry::Occupied(occupied) => {
                                    let occupied = occupied.into_mut();
                                    if occupied.date_expires.map(|exp| exp > now).unwrap_or(true) {
                                        occupied.merge(&claim, key.key.aggregation);
                                    } else {
                                        *occupied = claim;
                                    }
                                }
                                Entry::Vacant(vacant) => {
                                    vacant.insert(claim);
                                }
                            }
                        }
                    }
                }
                PlasmaUpdateV1::Chat {
                    admin,
                    alias,
                    authentic,
                    ip_address,
                    message,
                    recipient,
                    team_name,
                    visitor_id,
                    chat_id,
                    //player_id,
                    ..
                } => {
                    if let Some(realm) = self.realms.realm_mut(chat_id.arena_id.realm_id) {
                        let message = Arc::new(MessageDto {
                            alias,
                            authentic,
                            authority: admin,
                            team_name,
                            message,
                            visitor_id,
                            whisper: matches!(recipient, ChatRecipient::TeamOf(_)),
                        });
                        //let player_id = player_id.filter(|_| sender == self.server_id);
                        match recipient {
                            ChatRecipient::Broadcast => {
                                realm.realm_context.chat.broadcast_message(
                                    Arc::clone(&message),
                                    Some(MessageAttribution {
                                        chat_id,
                                        sender_ip: ip_address,
                                    }),
                                    realm.scene_repo.iter_mut().map(|(_, t)| &mut t.arena),
                                    None,
                                    true,
                                );
                            }
                            ChatRecipient::Arena => {
                                log::warn!("broadcast to scene is deprecated and will erroneously not set recent chat");
                                realm.realm_context.chat.broadcast_message(
                                    Arc::clone(&message),
                                    Some(MessageAttribution {
                                        chat_id,
                                        sender_ip: ip_address,
                                    }),
                                    realm
                                        .scene_repo
                                        .get_mut(chat_id.arena_id.scene_id)
                                        .map(|s| &mut s.arena),
                                    None,
                                    false,
                                );
                            }
                            ChatRecipient::Player(player_id) => {
                                if let Some(scene) =
                                    realm.scene_repo.get_mut(chat_id.arena_id.scene_id)
                                    && let Some(player) =
                                        scene.arena.arena_context.players.get_mut(player_id)
                                    && player.regulator.active()
                                    && let Some(client) = player.client_mut()
                                {
                                    client.chat.receive(
                                        &message,
                                        Some(MessageAttribution {
                                            chat_id,
                                            sender_ip: ip_address,
                                        }),
                                    );
                                }
                            }
                            ChatRecipient::TeamOf(player_id) => {
                                if let Some(scene) =
                                    realm.scene_repo.get_mut(chat_id.arena_id.scene_id)
                                    && let Some(player) =
                                        scene.arena.arena_context.players.get_mut(player_id)
                                    && player.regulator.active()
                                    && let Some(members) =
                                        scene.arena.arena_service.get_team_members(player_id)
                                {
                                    for member in members {
                                        if let Some(player) =
                                            scene.arena.arena_context.players.get_mut(member)
                                        {
                                            if let Some(client) = player.client_mut() {
                                                client.chat.receive(
                                                    &message,
                                                    Some(MessageAttribution {
                                                        chat_id,
                                                        sender_ip: ip_address,
                                                    }),
                                                );
                                            }
                                        } else {
                                            debug_assert!(
                                                false,
                                                "team member {:?} doesn't exist",
                                                member
                                            );
                                        }
                                    }
                                }
                            }
                            ChatRecipient::None => {}
                        }
                    }
                }
                PlasmaUpdateV1::Quests { fraction } => {
                    if fraction.is_finite() {
                        let fraction = fraction.clamp(0.0, 1.0);
                        if fraction != self.plasma.quest_fraction {
                            warn!("quest fraction changed to {fraction:.2}");
                        }
                        self.plasma.quest_fraction = fraction;
                    }
                }
                PlasmaUpdateV1::Domains { domains } => {
                    if self.server_id.kind.is_local() {
                        continue;
                    }
                    let prefix = G::GAME_CONSTANTS.domain.split_once('.').unwrap().0;
                    let domains = Vec::from(domains)
                        .into_iter()
                        .filter_map(|mut d| {
                            if !d.domain.starts_with(prefix) {
                                d.domain = DomainName::new(&format!("{prefix}.{}", d.domain))?;
                            }
                            Some(d)
                        })
                        .collect::<Box<[DomainDto]>>();

                    if let Some(domain_backup) = &self.plasma.domain_backup {
                        let json = serde_json::to_string(&domains).unwrap();
                        let path = domain_backup.clone();
                        tokio::spawn(async move {
                            let _ = tokio::fs::write(&*path, json).await;
                        });
                    }

                    self.system.alternative_domains = domains
                        .iter()
                        .filter(|d| &*d.domain != G::GAME_CONSTANTS.domain)
                        .map(|domain| domain.domain)
                        .collect();
                    *self.plasma.cors_alternative_domains.lock().unwrap() =
                        self.system.alternative_domains.clone();
                    if let Some((config, date_certificate_expires)) = load_domains::<G>(&*domains) {
                        self.plasma.rustls_config.reload_from_config(config);
                        self.plasma.date_certificate_expires = Some(date_certificate_expires);
                    }
                }
                PlasmaUpdateV1::Leaderboard {
                    period_id,
                    realm_id,
                    scores,
                } => {
                    if let Some(realm) = self.realms.realm_mut(realm_id) {
                        realm
                            .realm_context
                            .leaderboard
                            .put_leaderboard(period_id, scores);
                    }
                }
                PlasmaUpdateV1::Parley { sender, message } => {
                    if let Ok(message) = serde_json::from_value::<ServerMessage>(message) {
                        match message {
                            ServerMessage::Game {
                                sender_arena_id,
                                arena_id,
                                message,
                            } => {
                                if let Some(scene) = self.realms.get_mut(arena_id) {
                                    scene.arena.arena_service.server_message(
                                        sender,
                                        sender_arena_id,
                                        message,
                                        &mut scene.arena.arena_context,
                                    );
                                }
                            }
                            ServerMessage::Engine {
                                sender_arena_id,
                                arena_id,
                                redirected,
                            } => {
                                use base64::prelude::*;
                                let (arena_id, returning, accept_invitation_id) = self.resolve(
                                    arena_id,
                                    SendPlasmaRequest {
                                        web_socket: self.plasma.web_socket.sender.clone(),
                                        local: ctx.address().recipient(),
                                        local_server_id: self.server_id,
                                    },
                                );
                                debug_assert!(returning.is_none());
                                if let Some(scene) = self.realms.get_mut(arena_id)
                                    && let Ok(base64ed) = BASE64_STANDARD_NO_PAD.decode(&redirected)
                                    && let Ok(redirected) =
                                        decode_buffer::<RedirectedPlayer>(&base64ed)
                                {
                                    let player_id = scene.arena.arena_context.receive_player(
                                        sender,
                                        sender_arena_id,
                                        redirected,
                                    );
                                    scene.arena.arena_service.player_joined(
                                        player_id,
                                        &mut scene.arena.arena_context.players[player_id],
                                    );
                                    if accept_invitation_id.is_some() {
                                        let _ = self.invitations.accept(
                                            player_id,
                                            accept_invitation_id,
                                            &mut scene.arena.arena_context.players,
                                        );
                                    }
                                }
                            }
                            ServerMessage::Ack {
                                old_arena_id,
                                old_player_id,
                                old_token,
                                arena_id,
                                player_id,
                                token,
                            } => {
                                if let Some(scene) = self.realms.get_mut(old_arena_id) {
                                    if let Some(player) =
                                        scene.arena.arena_context.players.get_mut(old_player_id)
                                    {
                                        if let Some(client) = player.client_mut()
                                            && client.token == old_token
                                        {
                                            if let ClientStatus::Redirected {
                                                server_id,
                                                id_token: player_id_token,
                                                observer,
                                                ..
                                            } = &mut client.status
                                            {
                                                info!("finishing redirect of {old_player_id} connected={}", observer.is_some());
                                                if let Some(observer) = observer {
                                                    let msg = ObserverUpdate::Send {
                                                        message: CommonUpdate::Client(
                                                            ClientUpdate::Redirect {
                                                                server_id: *server_id,
                                                                arena_id,
                                                                player_id,
                                                                token,
                                                            },
                                                        ),
                                                        reliable: true,
                                                    };

                                                    if self.server_id.kind.is_local() {
                                                        let observer = observer.clone();
                                                        let delay = Duration::from_millis(
                                                            thread_rng().gen_range(0..250),
                                                        );
                                                        tokio::spawn(async move {
                                                            tokio::time::sleep(delay).await;
                                                            let _ = observer.send(msg);
                                                        });
                                                    } else {
                                                        let _ = observer.send(msg);
                                                    }
                                                }
                                                *player_id_token =
                                                    Some((arena_id, player_id, token));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                PlasmaUpdateV1::Player {
                    active_heartbeat,
                    admin,
                    arena_id,
                    moderator,
                    nick_name,
                    player_id,
                    session_token,
                    user,
                    visitor_id,
                    ..
                } => {
                    if let Some(scene) = self.realms.get_mut(arena_id) {
                        if let Some(player) = scene.arena.arena_context.players.get_mut(player_id) {
                            if let Some(client) = player.client_mut() {
                                if client.session.session_token == Some(session_token)
                                    || client.session.visitor_id == Some(visitor_id)
                                {
                                    client.session.active_heartbeat = active_heartbeat;
                                    client.session.user = user;
                                    client.session.visitor_id = Some(visitor_id);
                                    client.session.nick_name = nick_name;
                                    client.session.admin = admin;
                                    client.session.moderator = moderator;
                                    info!(
                                        "set moderator status of {session_token:?} to {moderator}"
                                    );
                                } else {
                                    warn!("user_id/session_id didn't match");
                                }
                            }
                        }
                    }
                }
                PlasmaUpdateV1::Role { role } => {
                    self.plasma.set_role(role);
                }
                PlasmaUpdateV1::Snippets { snippets } => {
                    // This is the first snippet update, so send to existing clients.
                    //
                    // Note: it is common for clients to connect before snippets load during
                    // debugging or after server crash.
                    let bootstrap = self.clients.js_snippets.is_empty() && !snippets.is_empty();
                    self.clients.js_snippets.clear();
                    let mut ads_txt = self.clients.ads_txt.write().unwrap();
                    ads_txt.clear();
                    for snippet in Vec::from(snippets) {
                        if let Some(game_id) = snippet.criteria.game_id {
                            if game_id != G::GAME_CONSTANTS.game_id() {
                                // defensive programming.
                                continue;
                            }
                        }
                        match snippet.name.as_str() {
                            "ads.txt" => {
                                ads_txt.insert(
                                    snippet.criteria.referrer,
                                    Bytes::from(snippet.content.into_bytes()),
                                );
                            }
                            "snippet.js" => {
                                self.clients.js_snippets.push((
                                    snippet.criteria,
                                    snippet.content.into_boxed_str().into(),
                                ));
                            }
                            "terms.md" => {
                                self.translations.update_terms(snippet.content);
                            }
                            "privacy.md" => {
                                self.translations.update_privacy(snippet.content);
                            }
                            _ => {
                                warn!("unrecognized snippet name {}", snippet.name)
                            }
                        }
                    }
                    if bootstrap {
                        let mut count = 0;
                        for (_, scene) in self.realms.iter() {
                            for (_, player) in scene.arena.arena_context.players.iter() {
                                let Some(client) = player.client() else {
                                    continue;
                                };
                                let ClientStatus::Connected { observer, .. } = &client.status
                                else {
                                    continue;
                                };
                                for snippet in Vec::from(self.clients.get_snippets(
                                    client.metrics.referrer,
                                    client.metrics.cohort_id,
                                    client.metrics.region_id,
                                    client.metrics.user_agent_id,
                                )) {
                                    let _ = observer.send(ObserverUpdate::Send {
                                        message: CommonUpdate::Client(
                                            ClientUpdate::BootstrapSnippet(snippet),
                                        ),
                                        reliable: true,
                                    });
                                }
                                count += 1;
                            }
                        }
                        if count > 0 {
                            println!("bootstrapped snippets for {count}");
                        }
                    }
                }
                PlasmaUpdateV1::Topology { servers } => {
                    // Trust plasma about our region id because our database may be out of date.
                    if let Some(this_server) = servers.get(&self.server_id) {
                        self.region_id = this_server.region_id;
                    }

                    self.plasma.servers = servers;
                    self.plasma.flush_arenas(
                        self.server_id,
                        ctx.address().recipient(),
                        &mut self.realms,
                    );
                    // TODO: ArenaId::default() is a hack. Should process on client.
                    let mut system_instances = self
                        .plasma
                        .servers
                        .iter()
                        .filter(|(server_id, server)| {
                            **server_id != self.server_id
                                && server.realm(RealmId::PublicDefault).is_some()
                        })
                        .flat_map(|(server_id, server)| {
                            server
                                .realm(RealmId::PublicDefault)
                                .unwrap()
                                .scenes
                                .iter()
                                .map(move |(scene_id, scene)| InstancePickerDto {
                                    scene_id: *scene_id,
                                    server_id: *server_id,
                                    player_count: scene.player_count,
                                    region_id: server.region_id,
                                    sanctioned: true,
                                })
                        })
                        .collect::<Vec<_>>();
                    if let Some(default) = self.realms.realm(RealmId::PublicDefault) {
                        for (scene_id, scene) in default.scene_repo.iter() {
                            system_instances.push(InstancePickerDto {
                                scene_id,
                                server_id: self.server_id,
                                region_id: self.region_id,
                                player_count: scene.arena.arena_context.players.real_players_live
                                    as u16,
                                sanctioned: self.plasma.is_sanctioned(
                                    self.server_id,
                                    ArenaId {
                                        realm_id: RealmId::PublicDefault,
                                        scene_id,
                                    },
                                ),
                            });
                        }
                    }

                    self.system.instances = system_instances.into();

                    let system_servers = self
                        .plasma
                        .servers
                        .iter()
                        .filter(|(server_id, server)| {
                            if **server_id == self.server_id {
                                self.realms.realm(RealmId::PublicDefault).is_some()
                            } else {
                                server.realm(RealmId::PublicDefault).is_some()
                            }
                        })
                        .map(|(server_id, server)| {
                            let (player_count,) = if *server_id == self.server_id {
                                //let realm = self.realms.realm(None).unwrap();
                                /*
                                let tier_numbers = realm
                                    .scene_repo
                                    .iter()
                                    .filter(|(scene_id, _)| {
                                        self.plasma.is_sanctioned(
                                            self.server_id,
                                            ArenaId {
                                                realm_id: None,
                                                scene_id: *scene_id,
                                            },
                                        )
                                    })
                                    .map(|(scene_id, _)| scene_id.tier_number)
                                    .collect();
                                */
                                let player_count = self
                                    .realms
                                    .iter()
                                    .map(|(_, s)| {
                                        s.arena.arena_context.players.real_players_live as u32
                                    })
                                    .sum();
                                (player_count,)
                            } else {
                                //let realm = server.realm(None).unwrap();
                                //let tier_numbers =
                                //    realm.scenes.keys().map(|s| s.tier_number).collect();
                                let player_count =
                                    server.arenas().map(|(_, s)| s.player_count as u32).sum();
                                (player_count,)
                            };
                            ServerPickerItem {
                                server_id: *server_id,
                                region_id: server.region_id,
                                datacenter: server.datacenter.clone(),
                                //tier_numbers,
                                player_count,
                            }
                        })
                        .collect::<Vec<_>>();

                    self.system.servers = system_servers.into();
                    self.system.available_servers = self.plasma.servers.keys().copied().collect();
                }
                PlasmaUpdateV1::Track {
                    no_referrer,
                    other_referrer,
                    referrers,
                } => {
                    self.metrics.no_referrer = no_referrer;
                    self.metrics.other_referrer = other_referrer;
                    self.metrics.tracked_referrers = referrers;
                }
                PlasmaUpdateV1::Translations { file_url } => {
                    let request = self.plasma.file_client.get(&file_url);
                    async move { request.send().await?.json::<TranslationsFile>().await }
                        .into_actor(self)
                        .map(move |res, act, _ctx| match res {
                            Ok(file) => {
                                act.translations.update(file.languages, file.translations);
                                info!("GET {file_url} OK");
                            }
                            Err(err) => {
                                error!("GET {file_url}: {err}");
                            }
                        })
                        .spawn(ctx);
                }
                _ => {}
            }
        }
        // warn!("unhandled plasma update {update:?}");
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ServerMessage {
    /// Game-initiated redirect.
    Game {
        sender_arena_id: ArenaId,
        arena_id: ArenaId,
        message: Value,
    },
    /// Engine-initiated redirect.
    Engine {
        sender_arena_id: ArenaId,
        arena_id: ArenaQuery,
        redirected: String,
    },
    Ack {
        old_arena_id: ArenaId,
        old_player_id: PlayerId,
        old_token: ReconnectionToken,
        arena_id: ArenaId,
        player_id: PlayerId,
        token: ReconnectionToken,
    },
}
