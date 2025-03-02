// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{ServerMessage, SystemActlet};
use crate::actor::{PlasmaActlet, ServerActor};
use crate::bitcode::{self, *};
use crate::net::{ActivePermit, IpRateLimiter};
use crate::observer::{ObserverMessage, ObserverMessageBody, ObserverUpdate};
use crate::rate_limiter::RateLimiterProps;
use crate::router::AllowedOrigin;
use crate::service::{
    ArenaService, ChatRepo, ClientChatData, ClientInvitationData, ClientMetricData,
    ClientQuestData, InvitationRepo, LeaderboardRepo, LiveboardRepo, MetricRepo, Player,
    PlayerInner, PlayerRepo, Realm, SendPlasmaRequest, ShardContextProvider,
};
use crate::{
    AdEvent, ArenaContext, ArenaEntry, ArenaId, ArenaQuery, ArenaSettingsDto, ArenaToken,
    BannerAdEvent, ClaimSubset, ClaimUpdateDto, ClaimValue, ClientActivity, ClientRequest,
    ClientUpdate, CohortId, CommonRequest, CommonUpdate, GameFence, InstancePickerDto,
    InvitationId, LanguageId, LeaderboardCaveat, LeaderboardUpdate, LifecycleId, LiveboardUpdate,
    NickName, NonZeroUnixMillis, PlasmaRequest, PlasmaRequestV1, PlayerId, PlayerUpdate,
    QuestEvent, QuestState, RealmId, ReconnectionToken, Referrer, RegionId, SceneId, ScopeClaimKey,
    ServerId, SessionToken, SnippetCriteria, SocketQuery, SystemUpdate, UnixTime, UserAgentId,
    VideoAdEvent, VisitorId,
};
use actix::{AsyncContext, Context as ActorContext, Handler, Message};
use bytes::Bytes;
use kodiak_common::rand::random;
use kodiak_common::{DomainName, NavigationMetricsDto};
use log::{error, info, warn};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::ops::{Deref, DerefMut};
use std::str::{self};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

/// Keeps track of clients a.k.a. real players a.k.a. websockets.
pub struct ClientActlet<G: ArenaService> {
    /// Claim updates from players that were pruned.
    pub(crate) trailing_claims: Vec<ClaimUpdateDto>,
    authenticate_rate_limiter: IpRateLimiter,
    pub(crate) js_snippets: Vec<(SnippetCriteria, Arc<str>)>,
    pub(crate) ads_txt: Arc<RwLock<HashMap<Option<Referrer>, Bytes>>>,
    _spooky: PhantomData<G>,
}

impl<G: ArenaService> ClientActlet<G> {
    pub fn new(
        authenticate: RateLimiterProps,
        ads_txt: Arc<RwLock<HashMap<Option<Referrer>, Bytes>>>,
    ) -> Self {
        Self {
            trailing_claims: Default::default(),
            authenticate_rate_limiter: authenticate.into(),
            js_snippets: Default::default(),
            ads_txt,
            _spooky: PhantomData,
        }
    }

    /// Client websocket (re)connected.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register(
        &mut self,
        player_id: PlayerId,
        register_observer: ClientAddr<G>,
        supports_unreliable: bool,
        players: &mut PlayerRepo<G>,
        leaderboard: &LeaderboardRepo<G>,
        liveboard: &LiveboardRepo<G>,
        metrics: &mut MetricRepo<G>,
        system: &SystemActlet<G>,
        server_id: ServerId,
        arena_id: ArenaId,
        game: &mut G,
    ) {
        let player_initializer = players.initializer();
        let player = match players.get_mut(player_id) {
            Some(player_tuple) => player_tuple,
            None => {
                // this can happen if the server is under extreme load.
                //debug_assert!(false, "client {player_id:?} gone in register");
                return;
            }
        };

        let client = match player.client_mut() {
            Some(client) => client,
            None => {
                debug_assert!(false, "register wasn't a client");
                return;
            }
        };

        if let ClientStatus::Redirected {
            server_id,
            id_token: player_id_token,
            observer,
            send_close,
            ..
        } = &mut client.status
        {
            if *send_close {
                if let &mut Some((arena_id, player_id, token)) = player_id_token {
                    let _ = register_observer.send(ObserverUpdate::Send {
                        message: CommonUpdate::Client(ClientUpdate::Redirect {
                            server_id: *server_id,
                            arena_id,
                            player_id,
                            token,
                        }),
                        reliable: true,
                    });
                }
                *observer = Some(register_observer);
            }
            return;
        }

        client.push_quest(QuestEvent::Socket {
            supports_unreliable,
            open: true,
        });
        client.metrics.last_unreliable = supports_unreliable;

        // Welcome the client in.
        let _ = register_observer.send(ObserverUpdate::Send {
            message: CommonUpdate::Client(ClientUpdate::SessionCreated {
                server_id,
                region_id: client.metrics.region_id,
                arena_id,
                player_id,
                token: client.token,
                date_created: client.metrics.date_created,
            }),
            reliable: true,
        });

        // Change status to connected.
        let mut active = None;

        Self::activate_client(
            &register_observer,
            client.ip_address,
            &mut client.chat,
            &client.invitation,
            &mut active,
            leaderboard,
            liveboard,
            player_initializer,
            system,
        );

        let new_status = ClientStatus::Connected {
            observer: register_observer.clone(),
            supports_unreliable,
            last_activity: Instant::now(),
            active,
            warn_if_dropped: true,
        };
        let old_status = std::mem::replace(&mut client.status, new_status);

        match old_status {
            ClientStatus::Connected { observer, .. } => {
                // If it still exists, old client is now retired.
                let _ = observer.send(ObserverUpdate::Close);
            }
            ClientStatus::Limbo { .. } => {
                info!("player {:?} restored from limbo", player_id);
            }
            ClientStatus::Pending { .. } => {
                metrics.start_visit(client);

                // We weren't in the game, so now we have to join.
                if player.regulator.join() {
                    game.player_joined(player_id, player);
                } else {
                    debug_assert!(false);
                }
            }
            ClientStatus::LeavingLimbo { .. } => {
                // We previously left the game, so now we have to rejoin.
                if player.regulator.join() {
                    game.player_joined(player_id, player);
                }
                info!("player {:?} restored from leaving limbo", player_id);
            }
            ClientStatus::Redirected { .. } => {
                unreachable!();
            }
        }
    }

    /// Client websocket disconnected.
    pub(crate) fn unregister(
        &mut self,
        player_id: PlayerId,
        unregister_observer: ClientAddr<G>,
        players: &mut PlayerRepo<G>,
    ) {
        // There is a possible race condition to handle:
        //  1. Client A registers
        //  3. Client B registers with the same session and player so evicts client A from limbo
        //  2. Client A unregisters and is placed in limbo

        let player = match players.get_mut(player_id) {
            Some(player) => player,
            None => return,
        };

        let client = match player.client_mut() {
            Some(client) => client,
            None => return,
        };

        match &mut client.status {
            ClientStatus::Connected {
                observer,
                supports_unreliable,
                ..
            } => {
                if observer.same_channel(&unregister_observer) {
                    let supports_unreliable = *supports_unreliable;
                    client.push_quest(QuestEvent::Socket {
                        supports_unreliable,
                        open: false,
                    });
                    client.status = ClientStatus::Limbo {
                        expiry: Instant::now() + G::LIMBO,
                    };
                    info!("player {:?} is in limbo", player_id);
                }
            }
            ClientStatus::Redirected { observer, .. } => {
                *observer = None;
            }
            _ => {}
        }
    }

    /// Makes the client active, if it isn't already.
    fn activate_client(
        observer: &UnboundedSender<ObserverUpdate<CommonUpdate<G::GameUpdate>>>,
        ip_address: IpAddr,
        chat: &mut ClientChatData,
        invitation: &ClientInvitationData,
        active: &mut Option<ActiveClientData<G>>,
        leaderboard: &LeaderboardRepo<G>,
        liveboard: &LiveboardRepo<G>,
        player_initializer: PlayerUpdate,
        system: &SystemActlet<G>,
    ) {
        if active.is_some() {
            return;
        }
        chat.inbox.mark_unread();
        let game_fence = random();
        *active = Some(ActiveClientData {
            data: G::ClientData::default(),
            game_fence,
            game_fence_done: false,
            activity: Default::default(),
            prev_claims: Default::default(),
            _permit: Some(ActivePermit::new(ip_address)),
        });

        let _ = observer.send(ObserverUpdate::Send {
            message: CommonUpdate::Client(ClientUpdate::ClearSyncState { game_fence }),
            reliable: true,
        });

        for initializer in leaderboard.initializers() {
            let _ = observer.send(ObserverUpdate::Send {
                message: CommonUpdate::Leaderboard(initializer),
                reliable: true,
            });
        }

        let _ = observer.send(ObserverUpdate::Send {
            // TODO
            message: CommonUpdate::Liveboard(liveboard.initializer(0, 0, None)),
            reliable: true,
        });

        let _ = observer.send(ObserverUpdate::Send {
            message: CommonUpdate::Player(player_initializer),
            reliable: true,
        });

        if let Some(initializer) = system.initializer() {
            let _ = observer.send(ObserverUpdate::Send {
                message: CommonUpdate::System(initializer),
                reliable: true,
            });
        }

        if let Some(initializer) = invitation.initializer() {
            let _ = observer.send(ObserverUpdate::Send {
                message: CommonUpdate::Invitation(initializer),
                reliable: true,
            });
        }
    }

    /// Update all clients with game state.
    #[allow(clippy::type_complexity)]
    pub(crate) fn update(
        &mut self,
        game: &G,
        players: &mut PlayerRepo<G>,
        liveboard: &mut LiveboardRepo<G>,
        leaderboard: &LeaderboardRepo<G>,
        server_delta: &Option<(Arc<[InstancePickerDto]>, Arc<[(ServerId, SceneId)]>)>,
        players_online: u32,
        server_id: ServerId,
        arena_id: ArenaId,
        plasma: &PlasmaActlet,
        system: &SystemActlet<G>,
        temporaries_available: bool,
    ) {
        let caveat =
            if !(plasma.is_sanctioned(server_id, arena_id) || arena_id.realm_id.is_temporary()) {
                Some(LeaderboardCaveat::Closing)
            } else if plasma.role.is_closing() {
                Some(LeaderboardCaveat::Closing)
            } else if arena_id.realm_id.is_temporary() {
                Some(LeaderboardCaveat::Temporary)
            } else if server_id.kind.is_cloud() && plasma.role.is_unlisted() {
                Some(LeaderboardCaveat::Unlisted)
            } else {
                None
            };

        let player_update = players.delta();
        let mut player_liveboard_update: HashMap<PlayerId, _> = players
            .iter()
            .filter(|(id, player)| {
                !id.is_bot()
                    && player
                        .client()
                        .map(|c| matches!(c.status, ClientStatus::Connected { .. }))
                        .unwrap_or(false)
            })
            .filter_map(|(player_id, _)| {
                liveboard
                    .your_score_nondestructive(player_id, game, &*players)
                    .map(|delta| (player_id, delta))
            })
            .collect();
        let leaderboard_update: Vec<_> = leaderboard.deltas_nondestructive().collect();

        let now = Instant::now();
        let player_initializer = players.initializer();
        let update_claims = !players.claim_update_rate_limit.should_limit_rate_with_now(
            &RateLimiterProps::new_pure(Duration::from_millis(200)),
            now,
        );
        let nz_now = NonZeroUnixMillis::now();
        let later_24h = nz_now.add_days(1);
        players
            .iter_mut()
            .for_each(move |(player_id, player): (PlayerId, &mut Player<G>)| {
                // Update client activity.
                let Some(client) = player.inner.client_mut() else {
                    return;
                };

                client.with_quest(|quest| {
                    quest.update_closing(caveat.is_some_and(|c| c.is_closing()));
                });

                if update_claims && client.claims_loaded() {
                    if matches!(client.status, ClientStatus::Connected{active: Some(ActiveClientData{activity: ClientActivity::Active, ..}), ..}) {
                        let days = client.claim_with_now(ScopeClaimKey::days(), nz_now).unwrap_or(ClaimValue{
                            value: 0,
                            date_expires: None,
                            date_updated: NonZeroUnixMillis::MIN,
                        });
                        // TODO: review add_signed_minutes() etc. - can timezone_offset be a negative i64?
                        if nz_now.add_signed_minutes(-(client.metrics.timezone_offset as i64)).floor_days() > days.date_updated.add_signed_minutes(-(client.metrics.timezone_offset as i64)).floor_days() {
                            let value = days.value.saturating_add(1);
                            info!("incrementing days of {:?} to {value}", client.session.visitor_id);
                            client.update_claim(ScopeClaimKey::days(), value, None);
                        }
                        let streak = client.claim_with_now(ScopeClaimKey::streak(), nz_now).unwrap_or(ClaimValue {
                            value: 0,
                            date_expires: None,
                            date_updated: NonZeroUnixMillis::MIN,
                        });
                        if nz_now.add_signed_minutes(-(client.metrics.timezone_offset as i64)).floor_days() > streak.date_updated.add_signed_minutes(-(client.metrics.timezone_offset as i64)).floor_days()
                        {
                            let midnight_tomorrow = later_24h.add_signed_minutes(-(client.metrics.timezone_offset as i64)).floor_days().add_days(1).add_signed_minutes(client.metrics.timezone_offset as i64);
                            let value = streak.value.saturating_add(1);
                            info!(
                                "incrementing streak of {:?} to {value} (expires at {midnight_tomorrow} in {} hours)",
                                client.session.visitor_id,
                                midnight_tomorrow.minutes_since(NonZeroUnixMillis::now()) / 60
                            );
                            client.update_claim(
                                ScopeClaimKey::streak(),
                                value,
                                // Facilitate streak freezes in the future.
                                Some(midnight_tomorrow).max(streak.date_expires),
                            );
                        }
                    }
                    if server_id.kind.is_cloud() && arena_id.realm_id.is_public_default() && caveat.is_none() && let Some(score) = player.liveboard.score.some() && score > 0 {
                        let high_score = client.claim_with_now(ScopeClaimKey::high_score(), nz_now).map(|v| v.value).unwrap_or(0);
                        if score as u64 > high_score {
                            client.update_claim(
                                ScopeClaimKey::high_score(),
                                score as u64,
                                None,
                            );
                        }
                    }
                }

                if let ClientStatus::Connected {
                    last_activity,
                    observer,
                    active,
                    ..
                } = &mut client.status
                {
                    // println!("{:?}", active.as_ref().map(|a| a.activity));
                    let duration = if active
                        .as_ref()
                        .map(|a| a.activity.is_hidden())
                        .unwrap_or(true)
                    {
                        Duration::from_secs(5)
                    } else {
                        Duration::from_secs(10)
                    };
                    if now - *last_activity <= duration {
                        // Active.
                        if let Some(active) = active {
                            if active.activity.is_active() {
                                if active._permit.is_none() {
                                    active._permit = Some(ActivePermit::new(client.ip_address));
                                }
                            } else {
                                active._permit = None;
                            }
                        } else {
                            info!("{} is no longer lurking", client.ip_address);
                            Self::activate_client(
                                &*observer,
                                client.ip_address,
                                &mut client.chat,
                                &client.invitation,
                                active,
                                leaderboard,
                                liveboard,
                                player_initializer.clone(),
                                system,
                            );
                        }
                    } else {
                        // Inactive.
                        if active.is_some() {
                            info!("{} is lurking", client.ip_address);
                            *active = None;
                        }
                    }
                }

                // Bot or client in limbo or will be soon (not connected, cannot send an update).
                /*
                if !player.regulator.active()
                    || !player.client().map(|c| c.is_active()).unwrap_or(false)
                {
                    return;
                }
                */

                let ClientStatus::Connected {
                    observer,
                    active: Some(active),
                    ..
                } = &mut client.status
                else {
                    return;
                };

                if update_claims {
                    let mut claim_diff = HashMap::new();
                    active.prev_claims.retain(|key, _| {
                        if client.session.claim_with_now(*key, nz_now).is_none() {
                            claim_diff.insert(*key, None);
                            false
                        } else {
                            true
                        }
                    });
                    for (key, _) in &client.session.claims.claims {
                        if let Some(claim) = client.session.claim_with_now(*key, nz_now)
                            && Some(&claim) != active.prev_claims.get(&key)
                        {
                            claim_diff.insert(*key, Some(claim));
                            active.prev_claims.insert(*key, claim);
                        }
                    }
                    if !claim_diff.is_empty() {
                        let _ = observer.send(ObserverUpdate::Send {
                            message: CommonUpdate::Client(ClientUpdate::UpdateClaims(claim_diff)),
                            reliable: true,
                        });
                    }
                }

                let chat_update = ChatRepo::<G>::player_delta(&mut client.chat);
                let update = game.get_game_update(player_id, player);
                let observer = if let ClientStatus::Connected { observer, .. } =
                    &player.client().unwrap().status
                {
                    observer
                } else {
                    unreachable!();
                };

                if let Some(update) = update {
                    let _ = observer.send(ObserverUpdate::Send {
                        message: CommonUpdate::Game(update),
                        reliable: true,
                    });
                }

                if let Some((added, removed)) = player_update.as_ref() {
                    let _ = observer.send(ObserverUpdate::Send {
                        message: CommonUpdate::Player(PlayerUpdate::Updated {
                            added: Arc::clone(added),
                            removed: Arc::clone(removed),
                        }),
                        reliable: true,
                    });
                }

                if let Some(your_score) = player_liveboard_update.remove(&player_id) {
                    let _ = observer.send(ObserverUpdate::Send {
                        message: CommonUpdate::Liveboard(LiveboardUpdate::Updated {
                            your_score,
                            liveboard: Arc::clone(liveboard.get()),
                            players_on_shard: liveboard.player_count,
                            shard_per_scene: <G::Shard as ShardContextProvider<G>>::PER_SCENE,
                            players_online,
                            caveat,
                            temporaries_available,
                        }),
                        reliable: true,
                    });
                }
                if let Some(chat_update) = chat_update {
                    let _ = observer.send(ObserverUpdate::Send {
                        message: CommonUpdate::Chat(chat_update.clone()),
                        reliable: true,
                    });
                }

                for &(period_id, leaderboard) in &leaderboard_update {
                    let _ = observer.send(ObserverUpdate::Send {
                        message: CommonUpdate::Leaderboard(LeaderboardUpdate::Updated(
                            period_id,
                            Arc::clone(leaderboard),
                        )),
                        reliable: true,
                    });
                }

                if let Some((added, removed)) = server_delta.as_ref() {
                    if !added.is_empty() {
                        let _ = observer.send(ObserverUpdate::Send {
                            message: CommonUpdate::System(SystemUpdate::Added(Arc::clone(added))),
                            reliable: true,
                        });
                    }
                    if !removed.is_empty() {
                        let _ = observer.send(ObserverUpdate::Send {
                            message: CommonUpdate::System(SystemUpdate::Removed(Arc::clone(removed))),
                            reliable: true,
                        });
                    }
                }
            });
    }

    /// Cleans up old clients. Rate limited internally.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prune(
        &mut self,
        service: &mut G,
        context: &mut ArenaContext<G>,
        invitations: &mut InvitationRepo<G>,
        metrics: &mut MetricRepo<G>,
        arena_id: ArenaId,
    ) {
        let now = Instant::now();

        const PRUNE: RateLimiterProps = RateLimiterProps::const_new(Duration::from_secs(1), 0);
        if context
            .prune_rate_limit
            .should_limit_rate_with_now(&PRUNE, now)
        {
            return;
        }

        let players = &mut context.players;

        let mut unregistering = 0usize;
        let mut in_limbo = 0usize;
        let mut leaving_limbo = 0usize;
        let mut redirected = 0usize;
        let mut pending = 0usize;

        let mut to_forget = Vec::new();
        for (player_id, player) in players.iter_mut() {
            if let Some(client_data) = player.inner.client_mut() {
                match &mut client_data.status {
                    ClientStatus::Connected {
                        observer,
                        active,
                        warn_if_dropped,
                        ..
                    } => {
                        if active.is_none() && observer.is_closed() {
                            unregistering += 1;
                            if std::mem::take(warn_if_dropped) {
                                warn!("observer channel dropped for {}", client_data.ip_address);
                            }
                        }
                        // Wait for transition to limbo via unregister, which is the "proper" channel.
                    }
                    ClientStatus::Limbo { expiry, .. } => {
                        in_limbo += 1;
                        if &now >= expiry {
                            client_data.status = ClientStatus::LeavingLimbo {
                                expiry: now,
                                ticks: 0,
                                warn_if_unforgettable: true,
                            };
                            if player.regulator.active() {
                                service.player_quit(player_id, player);
                            }
                            player.regulator.leave();
                        }
                    }
                    ClientStatus::LeavingLimbo {
                        expiry,
                        ticks,
                        warn_if_unforgettable,
                    } => {
                        leaving_limbo += 1;
                        if *ticks < 2 {
                            // Give the regulator a chance to work.
                            *ticks += 1;
                        } else if now < *expiry {
                            // A new connection is brewing.
                        } else if !player.regulator.can_forget() {
                            if std::mem::take(warn_if_unforgettable) {
                                warn!(
                                    "observer leaving limbo but cannot forget {}",
                                    client_data.ip_address
                                );
                            }
                            debug_assert!(false, "player leaving limbo but cannot forget");
                        } else {
                            metrics.stop_visit(&mut *player);
                            info!("player_id {:?} expired from limbo", player_id);

                            to_forget.push(player_id);
                        }
                    }
                    ClientStatus::Redirected {
                        expiry,
                        observer,
                        send_close,
                        ..
                    } => {
                        redirected += 1;
                        if now < *expiry {
                            // Not expired.
                        } else if !player.regulator.can_forget() {
                            debug_assert!(false, "player redirected but cannot forget");
                        } else {
                            info!("player_id {:?} expired from redirection", player_id);
                            if let Some(observer) = observer {
                                if std::mem::take(send_close) {
                                    let _ = observer.send(ObserverUpdate::Close);
                                }
                            } else {
                                to_forget.push(player_id);
                            }
                        }
                    }
                    ClientStatus::Pending { expiry } => {
                        pending += 1;
                        if &now > expiry {
                            // Not actually in game, so no cleanup required.
                            to_forget.push(player_id);
                        }
                    }
                }
            } else if player.regulator.can_forget() {
                to_forget.push(player_id);
            }
        }

        // println!("prune {arena_id:?} {:?}", players.iter().filter_map(|(pid, p)| p.client().map(|c| (pid, c))).map(|(pid, c)| (pid, &c.status)).collect::<HashMap<_, _>>());

        const WARNING: usize = 16;
        const WARN: RateLimiterProps = RateLimiterProps::const_new(Duration::from_secs(60), 0);
        if (unregistering >= WARNING
            || in_limbo >= WARNING
            || leaving_limbo >= WARNING
            || redirected >= WARNING
            || pending >= WARNING)
            && !context
                .prune_warn_rate_limit
                .should_limit_rate_with_now(&WARN, now)
        {
            warn!("abnormal client states {arena_id:?}: unregistering={unregistering}, in_limbo={in_limbo}, leaving_limbo={leaving_limbo}, redirected={redirected}, pending={pending}");
        }

        for player_id in to_forget {
            let player = players.forget(player_id, invitations);
            if self.trailing_claims.len() < 64
                && let Some(client) = player.client()
                && let Some(update) =
                    client.claim_update(NonZeroUnixMillis::now(), arena_id, player_id)
            {
                self.trailing_claims.push(update);
            }
        }
    }

    /// Handles [`G::Command`]'s.
    fn handle_game_command(
        player_id: PlayerId,
        command: G::GameRequest,
        game_fence: Option<GameFence>,
        service: &mut G,
        players: &mut PlayerRepo<G>,
    ) -> Result<Option<G::GameUpdate>, &'static str> {
        let player = players.get_mut(player_id).ok_or("nonexistent observer")?;
        if !player.regulator.active() {
            return Err("inactive observer");
        }
        // Game updates for all players are usually processed at once, but we also allow
        // one-off responses.

        if let Some(client) = player.client_mut() {
            if let ClientStatus::Connected {
                active: Some(active),
                ..
            } = &mut client.status
            {
                let new = game_fence == Some(active.game_fence);
                if new < active.game_fence_done {
                    return if game_fence.is_none() {
                        Err("client forgot fence")
                    } else {
                        Err("fence from the future")
                    };
                }
                active.game_fence_done = new;
            }
        } else {
            // Never called.
            debug_assert!(game_fence.is_none());
        }

        Ok(service.player_command(command, player_id, player))
    }

    fn login(
        players: &mut PlayerRepo<G>,
        arena_id: ArenaId,
        arena_token: ArenaToken,
        player_id: PlayerId,
        session_token: SessionToken,
        plasma: &PlasmaActlet,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        if let Some(player) = players.get_mut(player_id) {
            if player.client().is_some() {
                Self::reauthenticate(
                    player_id,
                    player,
                    Some(session_token),
                    arena_id,
                    arena_token,
                    plasma,
                );
                Ok(Some(ClientUpdate::LoggedIn(session_token)))
            } else {
                debug_assert!(false);
                Err("bot")
            }
        } else {
            Err("nonexistent observer")
        }
    }

    fn reauthenticate(
        player_id: PlayerId,
        player: &mut Player<G>,
        session_token: Option<SessionToken>,
        arena_id: ArenaId,
        arena_token: ArenaToken,
        plasma: &PlasmaActlet,
    ) {
        debug_assert!(player_id.is_client());
        if let Some(client) = player.client_mut()
            && session_token != client.session.session_token
        {
            client.session = Default::default();
            client.session.claims.date_synchronized = NonZeroUnixMillis::now();
            client.session.session_token = session_token;
            if let Some(session_token) = session_token {
                plasma.do_request(PlasmaRequestV1::AuthenticatePlayer {
                    arena_id,
                    arena_token,
                    player_id,
                    session_token,
                });
            }
        }
    }

    /// Record client frames per second (FPS) for statistical purposes.
    fn tally_ad(
        player_id: PlayerId,
        event: AdEvent,
        players: &mut PlayerRepo<G>,
        metrics: &mut MetricRepo<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = players.get_mut(player_id).ok_or("player doesn't exist")?;
        let client = player.client_mut().ok_or("only clients can tally ads")?;
        client.push_quest(QuestEvent::Ad { ad: event });

        metrics.mutate_with(
            move |metrics| {
                let metric = match event {
                    AdEvent::Banner(BannerAdEvent::Show) => Some(&mut metrics.banner_ads),
                    AdEvent::Interstitial(VideoAdEvent::Finish) => Some(&mut metrics.video_ads),
                    AdEvent::Rewarded(VideoAdEvent::Finish) => Some(&mut metrics.rewarded_ads),
                    _ => None,
                };
                if let Some(metric) = metric {
                    metric.increment();
                }
            },
            &client.metrics,
        );
        Ok(None)
    }

    /// Record client frames per second (FPS) for statistical purposes.
    fn tally_fps(
        player_id: PlayerId,
        fps: f32,
        players: &mut PlayerRepo<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = players.get_mut(player_id).ok_or("player doesn't exist")?;
        let client = player.client_mut().ok_or("only clients can tally fps")?;

        client.metrics.fps = sanitize_tps(fps);
        if let Some(fps) = client.metrics.fps {
            client.push_quest(QuestEvent::Fps { fps });
            Ok(None)
        } else {
            Err("invalid fps")
        }
    }

    fn record_quest_event(
        player_id: PlayerId,
        event: QuestEvent,
        players: &mut PlayerRepo<G>,
        metrics: &mut MetricRepo<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = players.get_mut(player_id).ok_or("player doesn't exist")?;
        let client = player.client_mut().ok_or("only clients can record nexus")?;
        if matches!(event, QuestEvent::Error { .. }) {
            metrics.mutate_with(
                |metrics| {
                    metrics.crashes.increment();
                },
                &client.metrics,
            );
        }
        if matches!(
            event,
            QuestEvent::Tutorial { .. }
                | QuestEvent::Nexus { .. }
                | QuestEvent::Error { .. }
                | QuestEvent::Trace { .. }
        ) {
            client.push_quest(event);
            Ok(None)
        } else {
            Err("disallowed quest event from client")
        }
    }

    fn announcement_preference(
        player_id: PlayerId,
        preference: bool,
        players: &mut PlayerRepo<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = players.get_mut(player_id).ok_or("player doesn't exist")?;
        let client = player
            .client_mut()
            .ok_or("only clients can have announcement preference")?;

        client.update_claim(
            ScopeClaimKey::announcement_preference(),
            preference as u64,
            None,
        );
        Ok(None)
    }

    fn switch_arena(
        player_id: PlayerId,
        server_id: ServerId,
        arena_id: ArenaQuery,
        service: &mut G,
        context: &mut ArenaContext<G>,
        metrics: &mut MetricRepo<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = context.players.get_mut(player_id).ok_or("missing player")?;
        if !player.regulator.active() {
            return Err("inactive");
        }
        let client = player.client_mut().ok_or("not a client")?;
        if !client.status.is_connected() {
            return Err("not connected");
        }
        player.regulator.leave();
        service.player_quit(player_id, player);
        if player.was_alive {
            player.was_alive = false;
            metrics.stop_play(player);
        }
        let redirected = context.send_player_impl(player_id, server_id, arena_id, false);
        let bitcoded = bitcode::encode(&redirected);
        use base64::prelude::*;
        let base64ed = BASE64_STANDARD_NO_PAD.encode(bitcoded);
        context
            .send_to_plasma
            .send(PlasmaRequest::V1(PlasmaRequestV1::SendServerMessage {
                recipients: std::iter::once(server_id).collect(),
                message: serde_json::to_value(ServerMessage::Engine {
                    sender_arena_id: context.topology.local_arena_id,
                    arena_id,
                    redirected: base64ed,
                })
                .unwrap(),
            }));
        Ok(None)
    }

    fn heartbeat(
        player_id: PlayerId,
        activity: ClientActivity,
        players: &mut PlayerRepo<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = players.get_mut(player_id).ok_or("player doesn't exist")?;
        let client = player
            .client_mut()
            .ok_or("only clients can send heartbeats")?;
        if let ClientStatus::Connected {
            active: Some(active),
            ..
        } = &mut client.status
        {
            active.activity = activity;
            client.push_quest(QuestEvent::Activity { activity });
        }
        Ok(None)
    }

    fn quit(
        &self,
        player_id: PlayerId,
        service: &mut G,
        players: &mut PlayerRepo<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = players.get_mut(player_id).ok_or("player doesn't exist")?;
        if player.regulator.active() {
            let was_alive = player.was_alive;
            if let Some(client) = player.client_mut() {
                client.with_quest(|quest| {
                    if !was_alive && !quest.quit {
                        quest.push(QuestEvent::State {
                            state: QuestState::Spawning {},
                        });
                    }
                    quest.quit = true;
                });
            }
            service.player_quit(player_id, player);
            Ok(None)
        } else {
            Err("inactive")
        }
    }

    fn arena_settings(
        &self,
        arena_id: ArenaId,
        player_id: PlayerId,
        arena_settings: String,
        arena_context: &mut ArenaContext<G>,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        let player = arena_context
            .players
            .get(player_id)
            .ok_or("player doesn't exist")?;
        let client = player.client().ok_or("not a client")?;
        if !(arena_id.realm_id.is_temporary() || arena_id.realm_id.is_named()) && !client.admin() {
            return Err("cannot configure arena");
        }
        let arena_settings =
            serde_json::from_str::<ArenaSettingsDto<G::ArenaSettings>>(&arena_settings)
                .map_err(|_| "invalid arena settings")?;

        arena_context.set_settings(arena_settings);
        Ok(None)
    }

    /// Handles an arbitrary [`ClientRequest`].
    #[allow(clippy::too_many_arguments)]
    fn handle_client_request(
        &mut self,
        arena_id: ArenaId,
        arena_token: ArenaToken,
        player_id: PlayerId,
        request: ClientRequest,
        service: &mut G,
        arena_context: &mut ArenaContext<G>,
        metrics: &mut MetricRepo<G>,
        plasma: &PlasmaActlet,
    ) -> Result<Option<ClientUpdate>, &'static str> {
        match request {
            ClientRequest::Login(session_token) => Self::login(
                &mut arena_context.players,
                arena_id,
                arena_token,
                player_id,
                session_token,
                plasma,
            ),
            ClientRequest::TallyAd(ad_type) => {
                Self::tally_ad(player_id, ad_type, &mut arena_context.players, metrics)
            }
            ClientRequest::TallyFps(fps) => {
                Self::tally_fps(player_id, fps, &mut arena_context.players)
            }
            ClientRequest::Heartbeat(client_activity) => {
                Self::heartbeat(player_id, client_activity, &mut arena_context.players)
            }
            ClientRequest::Quit => self.quit(player_id, service, &mut arena_context.players),
            ClientRequest::ArenaSettings(arena_settings) => {
                self.arena_settings(arena_id, player_id, arena_settings, arena_context)
            }
            ClientRequest::RecordQuestEvent(event) => {
                Self::record_quest_event(player_id, event, &mut arena_context.players, metrics)
            }
            ClientRequest::SwitchArena {
                server_id,
                arena_id,
            } => Self::switch_arena(
                player_id,
                server_id,
                arena_id,
                service,
                arena_context,
                metrics,
            ),
            ClientRequest::AnnouncementPreference(preference) => {
                Self::announcement_preference(player_id, preference, &mut arena_context.players)
            }
        }
    }

    /// Handles request made by real player.
    #[allow(clippy::too_many_arguments)]
    fn handle_observer_request(
        &mut self,
        player_id: PlayerId,
        request: CommonRequest<G::GameRequest>,
        arena_id: ArenaId,
        arena_token: ArenaToken,
        server_id: ServerId,
        realm: &mut Realm<G>,
        invitations: &mut InvitationRepo<G>,
        metrics: &mut MetricRepo<G>,
        plasma: &PlasmaActlet,
    ) -> Result<Option<CommonUpdate<G::GameUpdate>>, &'static str> {
        let scene = realm
            .scene_repo
            .get_mut(arena_id.scene_id)
            .ok_or("missing scene")?;
        {
            let player = match scene.arena.arena_context.players.get_mut(player_id) {
                Some(player) => player,
                None => {
                    debug_assert!(false, "{arena_id:?} {player_id:?} {request:?}");
                    return Err("no such player");
                }
            };

            let client = match player.client_mut() {
                Some(client) => client,
                None => {
                    debug_assert!(false);
                    return Err("not a client");
                }
            };

            if let ClientStatus::Connected { last_activity, .. } = &mut client.status {
                // Don't wake up. (not actually consequential)
                if !matches!(
                    request,
                    CommonRequest::Client(ClientRequest::Heartbeat(ClientActivity::Hidden))
                ) {
                    *last_activity = Instant::now();
                }
            } else {
                debug_assert!(
                    matches!(client.status, ClientStatus::Redirected { .. }),
                    "impossible due to synchronous nature of code (?)"
                );
            }
        }
        match request {
            // Goes first (fast path).
            CommonRequest::Game(command, game_fence) => Self::handle_game_command(
                player_id,
                command,
                game_fence,
                &mut scene.arena.arena_service,
                &mut scene.arena.arena_context.players,
            )
            .map(|u| u.map(CommonUpdate::Game)),
            CommonRequest::Client(request) => self
                .handle_client_request(
                    arena_id,
                    arena_token,
                    player_id,
                    request,
                    &mut scene.arena.arena_service,
                    &mut scene.arena.arena_context,
                    metrics,
                    plasma,
                )
                .map(|u| u.map(|u| CommonUpdate::Client(u))),
            CommonRequest::Chat(request) => realm
                .realm_context
                .chat
                .handle_chat_request(
                    arena_id,
                    player_id,
                    request,
                    &mut scene.arena,
                    metrics,
                    plasma,
                )
                .map(|u| Some(CommonUpdate::Chat(u))),
            CommonRequest::Invitation(request) => invitations
                .handle_invitation_request(
                    player_id,
                    request,
                    arena_id,
                    server_id,
                    &mut scene.arena.arena_context.players,
                )
                .map(|u| Some(CommonUpdate::Invitation(u))),
            CommonRequest::Redial { .. } => {
                debug_assert!(false);
                error!("unhandled redial");
                Ok(None)
            }
        }
    }

    /// Record network round-trip-time measured by websocket for statistical purposes.
    fn handle_observer_rtt(&mut self, player_id: PlayerId, rtt: u16, players: &mut PlayerRepo<G>) {
        let player = match players.get_mut(player_id) {
            Some(player) => player,
            None => return,
        };

        let client = match player.client_mut() {
            Some(client) => client,
            None => {
                debug_assert!(false);
                return;
            }
        };

        client.metrics.rtt = Some(rtt);

        client.push_quest(QuestEvent::Rtt { rtt });
    }

    pub(crate) fn get_snippets(
        &self,
        client_referrer: Option<Referrer>,
        client_cohort_id: CohortId,
        client_region_id: Option<RegionId>,
        client_user_agent_id: Option<UserAgentId>,
    ) -> Box<[Arc<str>]> {
        let mut ret = Vec::<Arc<str>>::new();
        for (criteria, content) in self.js_snippets.iter().filter(|(criteria, _)| {
            if let Some(referrer) = criteria.referrer
                && client_referrer != Some(referrer)
            {
                return false;
            }
            if let Some(cohort_id) = criteria.cohort_id
                && client_cohort_id != cohort_id
            {
                return false;
            }
            if let Some(region_id) = criteria.region_id
                && client_region_id != Some(region_id)
            {
                return false;
            }
            if let Some(user_agent_id) = criteria.user_agent_id
                && client_user_agent_id != Some(user_agent_id)
            {
                return false;
            }
            true
        }) {
            ret.push(content.clone());
            if !criteria.fallthrough {
                break;
            }
        }
        ret.into()
    }
}

/// Don't let bad values sneak in.
fn sanitize_tps(tps: f32) -> Option<f32> {
    tps.is_finite().then_some(tps.clamp(0.0, 144.0))
}

/// Data stored per client (a.k.a websocket a.k.a. real player).
#[derive(Debug)]
pub struct PlayerClientData<G: ArenaService> {
    /// Authentication.
    pub(crate) token: ReconnectionToken,
    /// Connection state.
    pub(crate) status: ClientStatus<G>,
    /// Plasma session.
    pub(crate) session: SessionData,
    /// Ip address.
    pub(crate) ip_address: IpAddr,
    /// Metrics-related information associated with each client.
    pub(crate) metrics: ClientMetricData,
    /// Invitation-related information associated with each client.
    pub(crate) invitation: ClientInvitationData,
    /// Chat-related information associated with each client.
    ///
    /// Exists regardless of connectedness or activity in order to persist
    /// private messages in case the client returns.
    pub(crate) chat: ClientChatData,
    /// Players this client has reported.
    pub(crate) reported: HashSet<IpAddr>,
}

impl<G: ArenaService> Deref for PlayerClientData<G> {
    type Target = ClientStatus<G>;

    fn deref(&self) -> &Self::Target {
        &self.status
    }
}

impl<G: ArenaService> DerefMut for PlayerClientData<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.status
    }
}

#[derive(Clone, Debug, Default, Encode, Decode)]
pub struct SessionData {
    /// Plasma session id.
    pub(crate) session_token: Option<SessionToken>,
    /// Plasma visitor id.
    pub(crate) visitor_id: Option<VisitorId>,
    /// Plasma nick name.
    pub(crate) nick_name: Option<NickName>,
    /// Include this visitor in the active set sent to plasma.
    pub active_heartbeat: bool,
    /// User, not just visitor.
    pub user: bool,
    /// Is admin (developer).
    pub(crate) admin: bool,
    /// Is moderator for in-game chat?
    pub(crate) moderator: bool,
    /// Claims received from plasma + local updates.
    pub(crate) claims: ClaimSubset,
}

impl SessionData {
    pub fn claim(&self, key: ScopeClaimKey) -> Option<ClaimValue> {
        self.claim_with_now(key, NonZeroUnixMillis::now())
    }

    pub fn claim_with_now(&self, key: ScopeClaimKey, now: NonZeroUnixMillis) -> Option<ClaimValue> {
        self.claims
            .claims
            .get(&key)
            .filter(|c| c.date_expires.map(|exp| exp > now).unwrap_or(true))
            .cloned()
    }
}

impl<G: ArenaService> PlayerClientData<G> {
    pub fn claims_loaded(&self) -> bool {
        self.session.visitor_id.is_some()
    }

    pub fn claim(&self, key: ScopeClaimKey) -> Option<ClaimValue> {
        self.session.claim(key)
    }

    pub fn claim_with_now(&self, key: ScopeClaimKey, now: NonZeroUnixMillis) -> Option<ClaimValue> {
        self.session.claim_with_now(key, now)
    }

    // TODO: Ensure update happens if player leaves before next heartbeat.
    pub fn update_claim(
        &mut self,
        key: ScopeClaimKey,
        value: u64,
        date_expires: Option<NonZeroUnixMillis>,
    ) {
        let value = ClaimValue {
            value,
            date_expires,
            date_updated: NonZeroUnixMillis::now(),
        };
        match self.session.claims.claims.entry(key) {
            Entry::Occupied(mut occupied) => {
                if occupied
                    .get()
                    .date_expires
                    .map(|exp| NonZeroUnixMillis::now() >= exp)
                    .unwrap_or(false)
                {
                    occupied.insert(value);
                } else {
                    occupied.into_mut().merge(&value, key.key.aggregation);
                }
            }
            Entry::Vacant(vacant) => {
                vacant.insert(value);
            }
        }
    }

    pub(crate) fn claim_update(
        &self,
        date_synchronized: NonZeroUnixMillis,
        arena_id: ArenaId,
        player_id: PlayerId,
    ) -> Option<ClaimUpdateDto> {
        let claims: HashMap<ScopeClaimKey, ClaimValue> = self
            .session
            .claims
            .iter()
            .filter_map(|(key, value)| {
                if value.date_updated < self.session.claims.date_synchronized {
                    return None;
                }
                self.session
                    .claims
                    .claims
                    .get(&key)
                    .cloned()
                    .map(|v| (*key, v))
            })
            .collect();
        if claims.is_empty() {
            return None;
        }
        Some(ClaimUpdateDto {
            visitor_id: self.session.visitor_id?,
            claims: ClaimSubset {
                claims,
                date_synchronized,
            },
            arena_id,
            player_id,
        })
    }

    /*
    pub fn with_claims(&mut self, callback: impl FnOnce(&mut ClaimSubset)) {
        let before = self.session.claims.clone();
        callback(&mut self.session.claims);
        if before != self.session.claims {
            // This will trigger sending to Plasma.
            self.session.claims.date_updated = NonZeroUnixMillis::now();
        }
    }
    */

    pub fn with_quest(&mut self, callback: impl FnOnce(&mut ClientQuestData)) {
        if let Some(quest) = &mut self.metrics.quest {
            callback(quest);
        }
    }

    pub fn push_quest_with(&mut self, event: impl FnOnce() -> QuestEvent) {
        self.with_quest(move |quest| {
            quest.push(event());
        })
    }

    pub fn push_quest(&mut self, event: QuestEvent) {
        self.with_quest(move |quest| {
            quest.push(event);
        })
    }

    pub fn region_id(&self) -> Option<RegionId> {
        self.metrics.region_id
    }

    pub(crate) fn nick_name(&self) -> Option<NickName> {
        self.session.nick_name
    }

    pub fn admin(&self) -> bool {
        self.session.admin
    }

    pub fn moderator(&self) -> bool {
        self.session.moderator
    }
}

#[derive(Debug)]
pub struct ActiveClientData<G: ArenaService> {
    /// Game specific client data. [`None`] if client is inactive.
    data: G::ClientData,
    game_fence: GameFence,
    game_fence_done: bool,
    activity: ClientActivity,
    prev_claims: HashMap<ScopeClaimKey, ClaimValue>,
    _permit: Option<ActivePermit>,
}

// TOOD: this was previously pub(crate)
pub enum ClientStatus<G: ArenaService> {
    /// Pending: Initial state. Visit not started yet. Can be forgotten after expiry.
    Pending { expiry: Instant },
    /// Connected and in game. Transitions to limbo if the connection is lost.
    Connected {
        observer: ClientAddr<G>,
        supports_unreliable: bool,
        /// Ensure we only warn once if the channel was dropped.
        warn_if_dropped: bool,
        /// Time of last message from client to server (initialized to client creation).
        last_activity: Instant,
        active: Option<ActiveClientData<G>>,
    },
    /// Disconnected but still in game (and visit still in progress).
    /// - Transitions to connected if a new connection is established.
    /// - Transitions to leaving limbo after expiry.
    Limbo { expiry: Instant },
    /// Disconnected and not in game (but visit still in progress).
    /// - Transitions to connected if a new connection is established.
    /// - Transitions to stale after finished leaving game.
    LeavingLimbo {
        expiry: Instant,
        ticks: u8,
        warn_if_unforgettable: bool,
    },
    /// Redirected and not in game (visit transferred to other server).
    /// - Transitions to leaving limbo after expiry.
    Redirected {
        expiry: Instant,
        server_id: ServerId,
        id_token: Option<(ArenaId, PlayerId, ReconnectionToken)>,
        observer: Option<ClientAddr<G>>,
        send_close: bool,
        active: Option<ActiveClientData<G>>,
    },
}

impl<G: ArenaService> Debug for ClientStatus<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected {
                observer,
                supports_unreliable,
                last_activity,
                active,
                ..
            } => f
                .debug_struct("Connected")
                .field("observer", &!observer.is_closed())
                .field("active", &active.is_some())
                .field("last_activity", &last_activity.elapsed().as_secs_f32())
                .field("supports_unreliable", supports_unreliable)
                .finish_non_exhaustive(),
            Self::Limbo { expiry } => f
                .debug_struct("Limbo")
                .field(
                    "expiry",
                    &expiry
                        .saturating_duration_since(Instant::now())
                        .as_secs_f32(),
                )
                .finish(),
            Self::LeavingLimbo { expiry, .. } => f
                .debug_struct("LeavingLimbo")
                .field(
                    "expiry",
                    &expiry
                        .saturating_duration_since(Instant::now())
                        .as_secs_f32(),
                )
                .finish_non_exhaustive(),
            Self::Pending { expiry } => f
                .debug_struct("Pending")
                .field(
                    "expiry",
                    &expiry
                        .saturating_duration_since(Instant::now())
                        .as_secs_f32(),
                )
                .finish(),
            Self::Redirected { expiry, .. } => f
                .debug_struct("Redirected")
                .field(
                    "expiry",
                    &expiry
                        .saturating_duration_since(Instant::now())
                        .as_secs_f32(),
                )
                .finish_non_exhaustive(),
        }
    }
}

impl<G: ArenaService> ClientStatus<G> {
    #[allow(unused)]
    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    #[allow(unused)]
    pub(crate) fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    #[allow(unused)]
    pub(crate) fn is_limbo(&self) -> bool {
        matches!(self, Self::Limbo { .. })
    }

    #[allow(unused)]
    pub(crate) fn is_leaving_limbo(&self) -> bool {
        matches!(self, Self::LeavingLimbo { .. })
    }

    pub(crate) fn is_redirected(&self) -> bool {
        matches!(self, Self::Redirected { .. })
    }

    pub fn data(&self) -> Option<&G::ClientData> {
        if let Self::Connected { active, .. } | Self::Redirected { active, .. } = self {
            active.as_ref().map(|a| &a.data)
        } else {
            None
        }
    }

    pub fn data_mut(&mut self) -> Option<&mut G::ClientData> {
        if let Self::Connected { active, .. } | Self::Redirected { active, .. } = self {
            active.as_mut().map(|a| &mut a.data)
        } else {
            None
        }
    }

    pub fn fenced_data(&self) -> Option<&G::ClientData> {
        if let Self::Connected { active, .. } | Self::Redirected { active, .. } = self {
            active
                .as_ref()
                .filter(|a| a.game_fence_done)
                .map(|a| &a.data)
        } else {
            None
        }
    }

    pub fn fenced_data_mut(&mut self) -> Option<&mut G::ClientData> {
        if let Self::Connected { active, .. } | Self::Redirected { active, .. } = self {
            active
                .as_mut()
                .filter(|a| a.game_fence_done)
                .map(|a| &mut a.data)
        } else {
            None
        }
    }
}

impl<G: ArenaService> PlayerClientData<G> {
    pub(crate) fn new(chat: ClientChatData, metrics: ClientMetricData, ip_address: IpAddr) -> Self {
        Self {
            token: random(),
            status: ClientStatus::Pending {
                expiry: Instant::now() + Duration::from_secs(10),
            },
            session: Default::default(),
            ip_address,
            metrics,
            invitation: Default::default(),
            reported: Default::default(),
            chat,
        }
    }

    /// Whether `send_with_reliable(_, false)` is actually unreliable.
    pub fn supports_unreliable(&self) -> bool {
        if let ClientStatus::Connected {
            supports_unreliable: web_transport,
            ..
        } = &self.status
        {
            *web_transport
        } else {
            false
        }
    }

    /// Send a (reliable) message to the client.
    ///
    /// # Panics
    ///
    /// If the client is not connected.
    pub fn send(&self, update: G::GameUpdate) {
        self.send_with_reliable(update, true);
    }

    /// Send an optionally-reliable message to the client.
    ///
    /// # Panics
    ///
    /// If the client is not connected.
    pub fn send_with_reliable(&self, update: G::GameUpdate, reliable: bool) {
        let observer = if let ClientStatus::Connected { observer, .. } = &self.status {
            observer
        } else {
            unreachable!();
        };
        let _ = observer.send(ObserverUpdate::Send {
            message: CommonUpdate::Game(update),
            reliable,
        });
    }

    /// Send (unreliable) trailer to client after redirecting it.
    pub fn send_trailer(&self, trailer: G::GameUpdate) {
        if let ClientStatus::Redirected { observer, .. } = &self.status {
            if let Some(observer) = observer {
                let _ = observer.send(ObserverUpdate::Send {
                    message: CommonUpdate::Game(trailer),
                    reliable: true,
                });
            }
        } else {
            debug_assert!(false);
        }
    }

    /// Client sent a message/heartbeat recently.
    pub fn is_active(&self) -> bool {
        if let ClientStatus::Connected { active, .. } = &self.status {
            active.is_some()
        } else {
            false
        }
    }
}

/// Handle client messages.
impl<G: ArenaService>
    Handler<ObserverMessage<CommonRequest<G::GameRequest>, CommonUpdate<G::GameUpdate>>>
    for ServerActor<G>
{
    type Result = ();

    fn handle(
        &mut self,
        msg: ObserverMessage<CommonRequest<G::GameRequest>, CommonUpdate<G::GameUpdate>>,
        _ctx: &mut Self::Context,
    ) {
        let Some((scene, realm_context)) = self.realms.get_mut_with_context(msg.arena_id) else {
            let typ = match &msg.body {
                ObserverMessageBody::Register { observer, .. } => {
                    let _ = observer.send(ObserverUpdate::Close);
                    "register"
                }
                ObserverMessageBody::Request { .. } => "request",
                ObserverMessageBody::RoundTripTime { .. } => "rtt",
                ObserverMessageBody::Unregister { .. } => "unregister",
            };
            error!("missing arena {:?} for {typ}", msg.arena_id);
            return;
        };

        match msg.body {
            ObserverMessageBody::Register {
                player_id,
                observer,
                supports_unreliable,
                ..
            } => {
                let shard_context =
                    ShardContextProvider::shard_context(&realm_context.per_realm, &scene.per_scene);
                self.clients.register(
                    player_id,
                    observer,
                    supports_unreliable,
                    &mut scene.arena.arena_context.players,
                    &realm_context.leaderboard,
                    &shard_context.liveboard,
                    &mut self.metrics,
                    &self.system,
                    self.server_id,
                    msg.arena_id,
                    &mut scene.arena.arena_service,
                )
            }
            ObserverMessageBody::Unregister {
                player_id,
                observer,
            } => {
                self.clients
                    .unregister(player_id, observer, &mut scene.arena.arena_context.players)
            }
            ObserverMessageBody::Request { player_id, request } => {
                match self.clients.handle_observer_request(
                    player_id,
                    request,
                    msg.arena_id,
                    scene.arena.arena_context.token,
                    self.server_id,
                    &mut self.realms.realm_mut(msg.arena_id.realm_id).unwrap(),
                    &mut self.invitations,
                    &mut self.metrics,
                    &self.plasma,
                ) {
                    Ok(Some(message)) => {
                        let scene = self.realms.get_mut(msg.arena_id).unwrap();
                        let player = match scene.arena.arena_context.players.get_mut(player_id) {
                            Some(player) => player,
                            None => {
                                debug_assert!(false);
                                return;
                            }
                        };

                        let client = match player.client() {
                            Some(client) => client,
                            None => {
                                debug_assert!(false);
                                return;
                            }
                        };

                        if let ClientStatus::Connected { observer, .. } = &client.status {
                            let _ = observer.send(ObserverUpdate::Send {
                                message,
                                reliable: true,
                            });
                        } else if !client.status.is_redirected() {
                            debug_assert!(false, "impossible due to synchronous nature of code");
                        }
                    }
                    Ok(None) => {}
                    Err(s) => {
                        warn!("observer request resulted in {}", s);
                    }
                }
            }
            ObserverMessageBody::RoundTripTime { player_id, rtt } => self
                .clients
                .handle_observer_rtt(player_id, rtt, &mut scene.arena.arena_context.players),
        }
    }
}

#[derive(Message, Debug)]
#[rtype(result = "Result<(ArenaId, PlayerId), ClientAuthErr>")]
pub struct ClientAuthRequest {
    /// Client ip address.
    pub ip_address: IpAddr,
    /// User agent.
    pub user_agent_id: Option<UserAgentId>,
    /// Referrer.
    pub referrer: Option<Referrer>,
    /// Desired arena id.
    pub arena_id: ArenaQuery,
    /// Session id.
    pub session_token: Option<SessionToken>,
    /// Previous cohort.
    pub cohort_id: CohortId,
    /// Selected language id.
    pub language_id: LanguageId,
    /// When joined the system (maybe now).
    pub date_created: NonZeroUnixMillis,
    /// Navigation performance measured by client.
    pub navigation: NavigationMetricsDto,
    /// Sanity-checked local timezone offset (minutes from UTC).
    pub timezone_offset: i16,
    /// Not a new visitor.
    pub lifecycle: LifecycleId,
    /// To track alterate domain metrics.
    pub alt_domain: Option<DomainName>,
}

#[derive(Debug, strum::IntoStaticStr)]
pub enum ClientAuthErr {
    TooManyRequests,
    UnsanctionedRealm,
    UnsanctionedArena,
    UnsanctionedServer,
    TooManyPlayers,
}

impl ClientAuthRequest {
    pub(crate) fn new<G: ArenaService>(
        query: SocketQuery,
        ip_address: IpAddr,
        origin: AllowedOrigin,
        user_agent_id: Option<UserAgentId>,
    ) -> Self {
        let now = NonZeroUnixMillis::now();

        Self {
            ip_address,
            referrer: origin
                .alternative_domain()
                .and_then(|d| Referrer::from_hostname(&*d, G::GAME_CONSTANTS.domain))
                .or(query.referrer),
            user_agent_id,
            arena_id: query.arena_id,
            session_token: query.session_token,
            lifecycle: if query.date_created.is_none() {
                LifecycleId::New
            } else {
                LifecycleId::Renewed
            },
            date_created: query
                .date_created
                .filter(|&d| d > NonZeroUnixMillis::from_i64(1680570365768) && d <= now)
                .unwrap_or(now),
            navigation: NavigationMetricsDto {
                dns: query.dns,
                tcp: query.tcp,
                tls: query.tls,
                http: query.http,
                dom: query.dom,
            },
            cohort_id: query.cohort_id,
            language_id: query.language_id,
            timezone_offset: query.timezone_offset.clamp(-12 * 60, 14 * 60),
            alt_domain: origin.alternative_domain(),
        }
    }
}

impl<G: ArenaService> ServerActor<G> {
    pub(crate) fn temporaries_available(&self) -> bool {
        self.realms
            .iter()
            .filter(|(arena_id, _)| arena_id.realm_id.is_temporary())
            .count()
            < G::MAX_TEMPORARY_SERVERS
            && !self.plasma.role.is_redirected()
    }
}

impl<G: ArenaService> ServerActor<G> {
    pub(crate) fn resolve(
        &mut self,
        mut msg_arena_id: ArenaQuery,
        send_plasma_request: SendPlasmaRequest,
    ) -> (ArenaId, Option<PlayerId>, Option<InvitationId>) {
        // Evaluate `NewTemporary` or `Invitation` into `Specific` or `AnyInstance`.
        let mut accept_invitation_id = None;
        match msg_arena_id {
            ArenaQuery::NewTemporary => {
                msg_arena_id = if self.temporaries_available()
                    && let Some(arena_id) = self
                        .invitations
                        .prepare_temporary_invitation(self.server_id)
                        .map(RealmId::Temporary)
                        .map(|realm_id| ArenaId::new(realm_id, Default::default()))
                {
                    let _ = self.realms.get_mut_or_default(
                        self.server_id,
                        arena_id,
                        send_plasma_request,
                    );
                    ArenaQuery::Specific(arena_id, None)
                } else {
                    ArenaQuery::default()
                }
            }
            ArenaQuery::Invitation(invitation_id) => {
                let arena_id = self.invitations.get(invitation_id).map(|id| id.arena_id);
                msg_arena_id = if let Some(arena_id) = arena_id {
                    accept_invitation_id = Some(invitation_id);
                    ArenaQuery::Specific(arena_id, None)
                } else {
                    ArenaQuery::default()
                };
            }
            _ => {}
        }

        let (arena_id, player_id) = match msg_arena_id {
            ArenaQuery::Specific(arena_id, player_id_reconnection_token)
                if let Some(scene) = self.realms.get(arena_id)
                    && let player_id = player_id_reconnection_token
                        .filter(|pidrt| scene.can_reconnect(*pidrt))
                        .map(|(p, _)| p)
                    && (player_id.is_some()
                        || arena_id.realm_id.is_temporary()
                        || self.plasma.is_sanctioned(self.server_id, arena_id)
                        || scene.arena.arena_context.last_sanctioned.elapsed()
                            < Duration::from_secs(120)) =>
            {
                (arena_id, player_id)
            }
            ArenaQuery::AnyInstance(realm_id, tier_number)
            | ArenaQuery::Specific(
                ArenaId {
                    realm_id,
                    scene_id: SceneId { tier_number, .. },
                },
                _,
            ) => {
                accept_invitation_id = None;
                let scene_id = self.realms.realm(realm_id).and_then(|realm| {
                    realm
                        .scene_repo
                        .iter()
                        .filter(|(scene_id, _)| {
                            realm_id.is_temporary()
                                || self.plasma.is_sanctioned(
                                    self.server_id,
                                    ArenaId::new(realm_id, *scene_id),
                                )
                        })
                        .map(|(scene_id, scene)| {
                            (
                                scene_id,
                                scene_id
                                    .tier_number
                                    .map(|n| n.0.get())
                                    .unwrap_or_default()
                                    .abs_diff(tier_number.map(|n| n.0.get()).unwrap_or_default()),
                                scene.arena.arena_context.players.real_players_live,
                            )
                        })
                        .min_by_key(|(_, tier_diff, p)| (*tier_diff, *p))
                        .map(|(s, _, _)| s)
                });
                (
                    scene_id
                        .map(|scene_id| ArenaId::new(realm_id, scene_id))
                        .unwrap_or_default(),
                    None,
                )
            }
            _ => (Default::default(), None),
        };

        (arena_id, player_id, accept_invitation_id)
    }
}

impl<G: ArenaService> Handler<ClientAuthRequest> for ServerActor<G> {
    type Result = Result<(ArenaId, PlayerId), ClientAuthErr>;

    fn handle(&mut self, msg: ClientAuthRequest, ctx: &mut ActorContext<Self>) -> Self::Result {
        let clients = &mut self.clients;

        if clients
            .authenticate_rate_limiter
            .should_limit_rate(msg.ip_address)
        {
            // Should only log IP of malicious actors.
            warn!("IP {:?} was rate limited", msg.ip_address);
            return Err(ClientAuthErr::TooManyRequests);
        }

        let (arena_id, player_id, accept_invitation_id) = self.resolve(
            msg.arena_id,
            SendPlasmaRequest {
                web_socket: self.plasma.web_socket.sender.clone(),
                local: ctx.address().recipient(),
                local_server_id: self.server_id,
            },
        );

        let Some((scene, realm_context)) = self.realms.get_mut_with_context(arena_id) else {
            return Err(ClientAuthErr::UnsanctionedArena);
        };
        let arena_token = scene.arena.arena_context.token;

        let player_id = if let Some(existing) = player_id {
            existing
        } else {
            // Deliberately allow new players when closing without redirecting, because that
            // seems safer.
            if self.plasma.role.is_redirected()
                && self
                    .plasma
                    .redirecting_since
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(5 * 60))
            {
                return Err(ClientAuthErr::UnsanctionedServer);
            }
            let mut i = 0;
            loop {
                let Some(player_id) = PlayerId::nth_client(i) else {
                    return Err(ClientAuthErr::TooManyPlayers);
                };
                if !scene.arena.arena_context.players.contains(player_id) {
                    break player_id;
                }
                i += 1;
            }
        };

        let player = match scene.arena.arena_context.players.entry(player_id) {
            ArenaEntry::Occupied(mut occupied) => {
                if let Some(client) = occupied.get_mut().client_mut() {
                    match &mut client.status {
                        ClientStatus::Pending { expiry }
                        | ClientStatus::Limbo { expiry, .. }
                        | ClientStatus::LeavingLimbo { expiry, .. }
                        | ClientStatus::Redirected { expiry, .. } => {
                            *expiry = (*expiry).max(Instant::now() + Duration::from_secs(5));
                        }
                        ClientStatus::Connected { .. } => {}
                    }
                    // Update the referrer, such that the correct snippet may be served.
                    client.metrics.update(&msg);
                    client.ip_address = msg.ip_address;
                } else {
                    debug_assert!(false, "impossible to be a bot since session was valid");
                }
                occupied.into_mut()
            }
            ArenaEntry::Vacant(vacant) => {
                let mut client_metric_data = ClientMetricData::new(
                    self.plasma.quest_fraction,
                    self.server_id,
                    arena_id,
                    &mut self.metrics.last_quest_date_created,
                    // TODO: add Copy.
                    msg.navigation.clone(),
                    msg.lifecycle,
                );
                client_metric_data.update(&msg);

                self.metrics.mutate_with(
                    |metrics| {
                        metrics.visitors.insert(&msg.date_created.to_i64());
                        metrics.alt_domain.push(msg.alt_domain.is_some());
                        metrics.dns.push(msg.navigation.dns as f32);
                        metrics.tcp.push(msg.navigation.tcp as f32);
                        metrics.tls.push(msg.navigation.tls as f32);
                        metrics.http.push(msg.navigation.http as f32);
                        metrics.dom.push(msg.navigation.dom as f32);
                    },
                    &client_metric_data,
                );

                let chat = realm_context
                    .chat
                    .initialize_client(self.server_id.number, arena_id);

                let client = PlayerClientData::new(chat, client_metric_data, msg.ip_address);

                let pd = Player::new(PlayerInner::Client(client));
                vacant.insert(pd)
            }
        };

        // If session added/changed, then register/re-register.
        //
        // TODO:
        // Before
        // 1) page loaded
        // 2) send old session
        // 3) send new session (flicker + bad renew)
        //
        // Now:
        // 1) page refreshed
        // 2) send no session
        // 3) send new session (bad renew)
        ClientActlet::reauthenticate(
            player_id,
            player,
            msg.session_token,
            arena_id,
            arena_token,
            &self.plasma,
        );

        // Re-accept invite (if it was valid).
        if accept_invitation_id.is_some() {
            let _ = self.invitations.accept(
                player_id,
                accept_invitation_id,
                &mut scene.arena.arena_context.players,
            );
        }

        Ok((arena_id, player_id))
    }
}

/// Directed to a websocket future corresponding to a client.
pub type ClientAddr<G> =
    UnboundedSender<ObserverUpdate<CommonUpdate<<G as ArenaService>::GameUpdate>>>;
