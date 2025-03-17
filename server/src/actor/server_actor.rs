// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::{AdminActlet, ClientActlet, PlasmaActlet, SystemActlet, TranslationActlet};
use crate::rate_limiter::RateLimiterProps;
use crate::service::{
    ArenaService, InvitationRepo, LeaderboardRepo, MetricRepo, RealmRepo, ShardContextProvider,
};
use crate::{ArenaId, ClientHash, PlasmaRequestV1, Referrer, RegionId, ServerId};
use actix::{Actor, AsyncContext, Context as ActorContext};
use axum_server::tls_rustls::RustlsConfig;
use bytes::Bytes;
use kodiak_common::DomainName;
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU8};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

/// An entire game server.
pub struct ServerActor<G: ArenaService> {
    /// What server/region does this server actor represent?
    pub(crate) server_id: ServerId,
    pub(crate) region_id: RegionId,

    /// API.
    pub(crate) plasma: PlasmaActlet,
    pub(crate) system: SystemActlet<G>,

    /// Game specific stuff.
    pub(crate) realms: RealmRepo<G>,
    /// Game client information.
    pub(crate) clients: ClientActlet<G>,
    pub(crate) translations: TranslationActlet,
    /// Shared invitations.
    pub(crate) invitations: InvitationRepo<G>,
    /// Shared admin interface.
    pub(crate) admin: AdminActlet<G>,
    /// Shared metrics.
    pub(crate) metrics: MetricRepo<G>,

    /// Drop missed updates.
    last_update: Instant,
    last_tick_end: Instant,

    /// Misc.
    stop_tx: Option<oneshot::Sender<()>>,
}

impl<G: ArenaService> Actor for ServerActor<G> {
    type Context = ActorContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("server actor started");

        // TODO: Investigate whether this only affects performance or can affect correctness.
        ctx.set_mailbox_capacity(50);

        ctx.run_interval(Duration::from_secs_f32(G::TICK_PERIOD_SECS), Self::update);

        self.plasma.set_infrastructure::<G>(
            G::GAME_CONSTANTS.game_id(),
            self.server_id,
            ctx.address().recipient(),
            &mut self.realms,
        );
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        error!("server actor stopping...");

        self.plasma.do_request(PlasmaRequestV1::UnregisterServer);

        /*
        tokio::spawn(async {
            let _ = tokio::time::sleep(Duration::from_millis(500)).await;

            // A process without this actor running should be restarted immediately.
            std::process::exit(0);
        });
        */

        // Do something to stop the process.
        // But not this, because https://github.com/actix/actix-net/issues/588
        /*
        if let Some(system) = actix::System::try_current() {
            log::warn!("stopping the actix system");
            system.stop();
        }
        */
        let _ = self.stop_tx.take().unwrap().send(());
    }
}

impl<G: ArenaService> ServerActor<G> {
    /// new returns a game server with the specified parameters.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        server_id: ServerId,
        redirect_server_number: &'static AtomicU8,
        client_hash: ClientHash,
        region_id: RegionId,
        bots: Option<u16>,
        ads_txt: Arc<RwLock<HashMap<Option<Referrer>, Bytes>>>,
        server_token: &'static AtomicU64,
        rustls_config: RustlsConfig,
        cors_alternative_domains: &'static Mutex<Arc<[DomainName]>>,
        domain_backup: Option<Arc<str>>,
        client_authenticate: RateLimiterProps,
        stop_tx: oneshot::Sender<()>,
    ) -> Self {
        let now = Instant::now();
        Self {
            server_id,
            region_id,
            clients: ClientActlet::new(client_authenticate, ads_txt),
            translations: TranslationActlet::default(),
            plasma: PlasmaActlet::new::<G>(
                redirect_server_number,
                server_token,
                rustls_config,
                cors_alternative_domains,
                domain_backup,
            ),
            system: SystemActlet::new(),
            admin: AdminActlet::new(client_hash),
            realms: RealmRepo::new(bots),
            invitations: InvitationRepo::default(),
            metrics: MetricRepo::new(),
            last_update: now,
            last_tick_end: now,
            stop_tx: Some(stop_tx),
        }
    }

    /// Call once every tick.
    pub fn update(&mut self, ctx: &mut <ServerActor<G> as Actor>::Context) {
        let now = Instant::now();
        if now.duration_since(self.last_update) < Duration::from_secs_f32(G::TICK_PERIOD_SECS * 0.5)
        {
            // Less than half a tick elapsed. Drop this update on the floor, to avoid jerking.
            return;
        }
        self.last_update = now;
        let server_delta = self.system.delta();
        let temporaries_available = self.temporaries_available();
        for (realm_id, context_realm) in self.realms.realms_mut() {
            let mut players_online = 0;
            for (_, scene) in context_realm.scene_repo.iter_mut() {
                players_online += scene.arena.arena_context.players.real_players_live as u32;
            }
            for (&server_id, server) in &self.plasma.servers {
                if server_id == self.server_id {
                    // Already added.
                    continue;
                }
                let Some(realm) = server.realm(realm_id) else {
                    continue;
                };
                for (_, scene) in &realm.scenes {
                    players_online += scene.player_count as u32;
                }
            }

            for (scene_id, scene) in context_realm.scene_repo.iter_mut() {
                let shard_context = <G::Shard as ShardContextProvider<G>>::shard_context_mut(
                    &mut context_realm.realm_context.per_realm,
                    &mut scene.per_scene,
                );
                let start = Instant::now();
                scene.arena.update(
                    &mut self.clients,
                    &mut shard_context.liveboard,
                    &context_realm.realm_context.leaderboard,
                    &mut self.invitations,
                    &mut context_realm.realm_context.chat,
                    &mut self.metrics,
                    &server_delta,
                    players_online,
                    self.server_id,
                    ArenaId::new(realm_id, scene_id),
                    &self.plasma,
                    &self.system,
                    temporaries_available,
                );
                if scene.arena.arena_context.tick_duration.count / 32
                    > (1.0 / G::TICK_PERIOD_SECS) as u32
                {
                    scene.arena.arena_context.tick_duration = Default::default();
                }
                scene
                    .arena
                    .arena_context
                    .tick_duration
                    .push(start.elapsed().as_secs_f32());
            }

            context_realm.realm_context.leaderboard.clear_deltas();

            for (_, scene) in context_realm.scene_repo.iter_mut() {
                if let Some(scene_shard_context) =
                    <G::Shard as ShardContextProvider<G>>::scene_shard_context_mut(
                        &mut scene.per_scene,
                    )
                {
                    context_realm
                        .realm_context
                        .leaderboard
                        .update(&scene_shard_context.liveboard, &self.plasma);
                    scene_shard_context.liveboard.update(&mut scene.arena);
                }
            }
            if let Some(realm_shard_context) =
                <G::Shard as ShardContextProvider<G>>::realm_shard_context_mut(
                    &mut context_realm.realm_context.per_realm,
                )
            {
                context_realm
                    .realm_context
                    .leaderboard
                    .update(&realm_shard_context.liveboard, &self.plasma);
                realm_shard_context
                    .liveboard
                    .update(&mut context_realm.scene_repo);
            }
        }

        let tick_end = Instant::now();
        let elapsed = tick_end.duration_since(self.last_tick_end).as_secs_f32();
        self.last_tick_end = tick_end;
        if elapsed > G::TICK_PERIOD_SECS * 4.0 {
            error!("long tick lasted: {elapsed:.2}s");
        } else if elapsed > G::TICK_PERIOD_SECS * 2.0 {
            warn!("long tick lasted: {elapsed:.2}s");
        }
        // Use separate [`Health`] instances so neither use case has gaps.
        self.metrics.health.record_tick::<G>(tick_end, elapsed);
        self.plasma.health.record_tick::<G>(tick_end, elapsed);

        // These are all rate-limited internally.
        LeaderboardRepo::update_to_plasma(self);
        MetricRepo::update_to_plasma(self, ctx);
        self.plasma.update(
            self.server_id,
            &mut self.realms,
            &mut self.clients,
            &mut self.metrics,
            self.region_id,
            self.admin.client_hash,
            ctx,
        );
        self.realms
            .collect_arenas(self.server_id, &mut self.invitations, &self.plasma);
    }
}
