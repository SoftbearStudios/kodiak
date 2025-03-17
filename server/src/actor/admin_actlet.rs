// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::ServerActor;
use crate::service::{ArenaService, Bundle, MetricBundle, MetricRepo, PlayerRepo, Score};
use crate::{
    AdminPlayerDto, AdminRequest, AdminUpdate, ClientHash, EngineMetrics, MetricFilter,
    PlayerAlias, PlayerId, RealmId, RegionId, SceneId, UserAgentId,
};
use actix::{fut, ActorFutureExt, Handler, ResponseActFuture, WrapFuture};
use std::collections::HashMap;
use std::hash::Hash;
use std::iter;
use std::marker::PhantomData;
use std::time::Duration;

/// Responsible for the admin interface.
pub struct AdminActlet<G: ArenaService> {
    pub(crate) client_hash: ClientHash,
    #[cfg(unix)]
    cpu_profile: Option<pprof::ProfilerGuard<'static>>,
    heap_profile: Option<dhat::Profiler>,
    _spooky: PhantomData<G>,
}

impl<G: ArenaService> AdminActlet<G> {
    pub fn new(client_hash: ClientHash) -> Self {
        Self {
            client_hash,
            #[cfg(unix)]
            cpu_profile: None,
            heap_profile: None,
            _spooky: PhantomData,
        }
    }

    /// (Temporarily) overrides the alias of a given real player.
    fn override_player_alias(
        &self,
        player_id: PlayerId,
        alias: PlayerAlias,
        service: &mut G,
        players: &mut PlayerRepo<G>,
    ) -> Result<AdminUpdate, &'static str> {
        let player = players.get_mut(player_id).ok_or("nonexistent player")?;
        if !player.regulator.active() {
            return Err("inactive");
        }
        // We still censor, in case of unauthorized admin access.
        let censored = PlayerAlias::new_sanitized(alias.as_str());
        service.override_alias(player_id, alias);
        player.alias = censored;
        Ok(AdminUpdate::PlayerAliasOverridden(censored))
    }

    /// (Temporarily) overrides the moderator status of a given real player.
    fn override_player_moderator(
        &self,
        player_id: PlayerId,
        moderator: bool,
        players: &mut PlayerRepo<G>,
    ) -> Result<AdminUpdate, &'static str> {
        let player = players.get_mut(player_id).ok_or("nonexistent player")?;
        let client = player.client_mut().ok_or("not a real player")?;
        client.session.moderator = moderator;
        Ok(AdminUpdate::PlayerModeratorOverridden(moderator))
    }

    fn request_category_inner<T: Hash + Eq + Copy>(
        &self,
        initial: impl IntoIterator<Item = T>,
        extract: impl Fn(&Bundle<EngineMetrics>) -> &HashMap<T, EngineMetrics>,
        metrics: &MetricRepo<G>,
    ) -> Box<[(T, f32)]> {
        let initial = initial.into_iter();
        let mut hash: HashMap<T, u32> = HashMap::with_capacity(initial.size_hint().0);
        for tracked in initial {
            hash.insert(tracked, 0);
        }
        let mut total = 0u32;
        for bundle in iter::once(&metrics.current).chain(metrics.history.iter()) {
            for (&key, metrics) in extract(&bundle.bundle).iter() {
                *hash.entry(key).or_default() += metrics.visits.total;
            }
            total += bundle.bundle.total.visits.total;
        }
        let mut list: Vec<(T, u32)> = hash.into_iter().collect();
        // Sort in reverse so higher counts are first.
        list.sort_unstable_by_key(|(_, count)| u32::MAX - count);
        let mut percents: Vec<_> = list
            .into_iter()
            .map(|(v, count)| (v, count as f32 / total as f32))
            .collect();
        percents.truncate(20);
        percents.into_boxed_slice()
    }

    /// Request metric data points for the last 24 calendar hours (excluding the current hour, in
    /// which metrics are incomplete).
    fn request_day(
        metrics: &MetricRepo<G>,
        filter: Option<MetricFilter>,
    ) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::DayRequested(
            metrics
                .history
                .oldest_ordered()
                .map(|bundle| (bundle.start, bundle.data_point(filter)))
                .collect(),
        ))
    }

    /// Get list of games hosted by the server.
    fn request_games(&self) -> Result<AdminUpdate, &'static str> {
        // We only support one game type per server.
        Ok(AdminUpdate::GamesRequested(
            vec![(G::GAME_CONSTANTS.game_id(), 1.0)].into_boxed_slice(),
        ))
    }

    /// Get admin view of real players in the game.
    fn request_players(&self, players: &PlayerRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::PlayersRequested(
            players
                .iter()
                .filter_map(|(player_id, player)| {
                    if let Some(client) = player.client().filter(|_| !player.is_out_of_game()) {
                        Some(AdminPlayerDto {
                            alias: player.alias,
                            player_id,
                            team_id: player.team_id,
                            region_id: client.metrics.region_id,
                            session_token: client.session.session_token,
                            ip_address: client.ip_address,
                            moderator: client.moderator(),
                            score: if let Score::Some(score) = player.liveboard.score {
                                score
                            } else {
                                0
                            },
                            plays: client.metrics.plays,
                            fps: client.metrics.fps,
                            rtt: client.metrics.rtt,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
        ))
    }

    /// Request a list of regions, sorted by percentage.
    fn request_regions(&self, metrics: &MetricRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::RegionsRequested(self.request_category_inner(
            RegionId::iter(),
            |bundle| &bundle.by_region_id,
            metrics,
        )))
    }

    /// Request a list of referrers, sorted by percentage, and truncated to a reasonable limit.
    fn request_referrers(&self, metrics: &MetricRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::ReferrersRequested(
            self.request_category_inner(
                // TODO: support aliases.
                metrics.tracked_referrers.keys().copied(),
                |bundle| &bundle.by_referrer,
                metrics,
            ),
        ))
    }

    /// Request summary of metrics for the current calendar calendar hour.
    fn request_summary(
        infrastructure: &mut ServerActor<G>,
        filter: Option<MetricFilter>,
    ) -> Result<AdminUpdate, &'static str> {
        let current = MetricRepo::get_metrics(infrastructure, filter);

        // One hour.
        // MetricRepo::get_metrics(infrastructure, filter).summarize(),
        let mut summary = infrastructure
            .metrics
            .history
            .oldest_ordered()
            .map(|bundle: &MetricBundle| bundle.metric(filter))
            .chain(iter::once(current.clone()))
            .sum::<EngineMetrics>()
            .summarize();

        // TODO: Make special [`DiscreteMetric`] that handles data that is not necessarily unique.
        summary.arenas_cached.total = current.arenas_cached.total;
        summary.invitations_cached.total = current.invitations_cached.total;
        summary.players_cached.total = current.players_cached.total;
        summary.sessions_cached.total = current.sessions_cached.total;

        Ok(AdminUpdate::SummaryRequested(Box::new(summary)))
    }

    /// Request a list of user agents, sorted by percentage.
    fn request_user_agents(&self, metrics: &MetricRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::UserAgentsRequested(
            self.request_category_inner(
                UserAgentId::iter(),
                |bundle| &bundle.by_user_agent_id,
                metrics,
            ),
        ))
    }

    fn start_cpu_profile(&mut self) -> Result<(), &'static str> {
        #[cfg(not(unix))]
        return Err("profile only available on Unix");

        #[cfg(unix)]
        if self.cpu_profile.is_some() {
            Err("profile already started")
        } else {
            self.cpu_profile =
                Some(pprof::ProfilerGuard::new(1000).map_err(|_| "failed to start profile")?);
            Ok(())
        }
    }

    fn start_heap_profile(&mut self) -> Result<(), &'static str> {
        if self.heap_profile.is_some() {
            Err("profile already started")
        } else {
            self.heap_profile = Some(dhat::Profiler::builder().trim_backtraces(Some(16)).build());
            Ok(())
        }
    }

    fn finish_cpu_profile(&mut self) -> Result<AdminUpdate, &'static str> {
        #[cfg(not(unix))]
        return Err("profile only available on Unix");

        #[cfg(unix)]
        if let Some(profile) = self.cpu_profile.take() {
            if let Ok(report) = profile.report().build() {
                let mut buf = Vec::new();
                report
                    .flamegraph(&mut buf)
                    .map_err(|_| "error writing profiler flamegraph")?;

                Ok(AdminUpdate::CpuProfileRequested(
                    String::from_utf8(buf).map_err(|_| "profile contained invalid utf8")?,
                ))
            } else {
                Err("error building profile report")
            }
        } else {
            Err("profile not started or was interrupted")
        }
    }

    fn finish_heap_profile(&mut self) -> Result<AdminUpdate, &'static str> {
        if let Some(mut profile) = self.heap_profile.take() {
            let output = profile.drop_and_get_memory_output();
            // Don't run Drop.
            std::mem::forget(profile);
            Ok(AdminUpdate::HeapProfileRequested(output))
        } else {
            Err("profile not started or was interrupted")
        }
    }
}

impl<G: ArenaService> Handler<AdminRequest> for ServerActor<G> {
    type Result = ResponseActFuture<Self, Result<AdminUpdate, &'static str>>;

    fn handle(&mut self, request: AdminRequest, _ctx: &mut Self::Context) -> Self::Result {
        match request {
            AdminRequest::OverridePlayerAlias { player_id, alias } => {
                Box::pin(fut::ready(if let Some(tier) = self.realms.main_mut() {
                    self.admin.override_player_alias(
                        player_id,
                        alias,
                        &mut tier.arena.arena_service,
                        &mut tier.arena.arena_context.players,
                    )
                } else {
                    Err("no main arena")
                }))
            }
            AdminRequest::OverridePlayerModerator {
                player_id,
                moderator,
            } => Box::pin(fut::ready(if let Some(tier) = self.realms.main_mut() {
                self.admin.override_player_moderator(
                    player_id,
                    moderator,
                    &mut tier.arena.arena_context.players,
                )
            } else {
                Err("no main arena")
            })),
            AdminRequest::RequestDay { filter } => {
                Box::pin(fut::ready(AdminActlet::request_day(&self.metrics, filter)))
            }
            AdminRequest::RequestGames => Box::pin(fut::ready(self.admin.request_games())),
            AdminRequest::RequestPlayers => Box::pin(fut::ready(
                if let Some(realm) = self.realms.realm(RealmId::PublicDefault) {
                    if let Some(scene) = realm.scene_repo.get(&SceneId::default()) {
                        self.admin
                            .request_players(&scene.arena.arena_context.players)
                    } else {
                        Err("no main scene")
                    }
                } else {
                    Err("no main realm")
                },
            )),
            AdminRequest::RequestRegions => {
                Box::pin(fut::ready(self.admin.request_regions(&self.metrics)))
            }
            AdminRequest::RequestReferrers => {
                Box::pin(fut::ready(self.admin.request_referrers(&self.metrics)))
            }
            AdminRequest::RequestSeries { .. } => {
                Box::pin(Box::pin(fut::ready(Err("failed to load"))))
            }
            AdminRequest::RequestCpuProfile(seconds) => {
                if let Err(e) = self.admin.start_cpu_profile() {
                    Box::pin(fut::ready(Err(e)))
                } else {
                    Box::pin(
                        tokio::time::sleep(Duration::from_secs(seconds as u64))
                            .into_actor(self)
                            .map(move |_res, act, _ctx| act.admin.finish_cpu_profile()),
                    )
                }
            }
            AdminRequest::RequestHeapProfile(seconds) => {
                if let Err(e) = self.admin.start_heap_profile() {
                    Box::pin(fut::ready(Err(e)))
                } else {
                    Box::pin(
                        tokio::time::sleep(Duration::from_secs(seconds as u64))
                            .into_actor(self)
                            .map(move |_res, act, _ctx| act.admin.finish_heap_profile()),
                    )
                }
            }
            AdminRequest::RequestServerId => Box::pin(fut::ready(Ok(
                AdminUpdate::ServerIdRequested(self.server_id),
            ))),
            AdminRequest::RequestSummary { filter } => {
                Box::pin(fut::ready(AdminActlet::request_summary(self, filter)))
            }
            AdminRequest::RequestUserAgents => {
                Box::pin(fut::ready(self.admin.request_user_agents(&self.metrics)))
            }
        }
    }
}
