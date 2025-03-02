// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::{ClientAuthRequest, Health, PlayerClientData, ServerActor};
use crate::bitcode::{self, *};
use crate::entry_point::HTTP_RATE_LIMITER;
use crate::net::ip_to_region_id;
use crate::service::{ArenaService, ClientQuestData, Player, Score};
use crate::{
    unwrap_or_return, ArenaId, CohortId, EngineMetrics, EngineMetricsDataPointDto, LanguageId,
    LifecycleId, MetricFilter, NonZeroUnixMillis, PlasmaRequestV1, QuestEvent, QuestSampleDto,
    QuestState, Referrer, RegionId, ServerId, UnixTime, UserAgentId,
};
use actix::{ActorFutureExt, Context as ActorContext, ContextFutureSpawner, WrapFuture};
use kodiak_common::heapless::HistoryBuffer;
use kodiak_common::rand::{thread_rng, Rng};
use kodiak_common::NavigationMetricsDto;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::iter;
use std::marker::PhantomData;
use std::time::Duration;
use tokio::runtime::Handle;

// TODO: was pub(crate)
/// Stores and updates metrics to increase observability.
pub struct MetricRepo<G: ArenaService> {
    pub(crate) no_referrer: Option<Referrer>,
    pub(crate) other_referrer: Option<Referrer>,
    pub(crate) tracked_referrers: HashMap<Referrer, Referrer>,
    pub(crate) startup: NonZeroUnixMillis,
    next_update: NonZeroUnixMillis,
    next_swap: NonZeroUnixMillis,
    pub(crate) current: MetricBundle,
    pub history: HistoryBuffer<MetricBundle, 24>,
    pub(crate) health: Health,
    pub(crate) pending_quests: Vec<QuestSampleDto>,
    pub(crate) last_quest_date_created: NonZeroUnixMillis,
    _spooky: PhantomData<G>,
}

/// Metric related data stored per client.
#[derive(Debug, Clone, Encode, Decode)]
pub struct ClientMetricData {
    /// Randomly assigned cohort.
    pub cohort_id: CohortId,
    /// Selected language.
    pub language_id: LanguageId,
    /// Sanity-checked local timezone offset (minutes from UTC).
    pub timezone_offset: i16,
    /// Summary of domain that referred client.
    pub referrer: Option<Referrer>,
    /// General geographic location of the client.
    pub region_id: Option<RegionId>,
    /// Client user agent high level id.
    pub user_agent_id: Option<UserAgentId>,
    /// Frames per second.
    pub fps: Option<f32>,
    /// Milliseconds of network a.k.a. latency round trip time.
    pub rtt: Option<u16>,
    /// For statistics purposes.
    pub date_created: NonZeroUnixMillis,
    /// Renewed, as opposed to new, session.
    pub lifecycle: LifecycleId,
    /// Ever accepted an invitation.
    pub invited: bool,
    /// When this session was created, for metrics purposes.
    pub created: NonZeroUnixMillis,
    /// When the current play was started, for metrics purposes.
    pub play_started: Option<NonZeroUnixMillis>,
    /// When the last play was stopped, for metrics purposes.
    pub play_stopped: Option<NonZeroUnixMillis>,
    /// When the current visit was started.
    pub visit_started: Option<NonZeroUnixMillis>,
    /// When the current visit was stopped.
    pub visit_stopped: Option<NonZeroUnixMillis>,
    /// How many plays on this session, for database purposes.
    pub plays: u32,
    /// How many plays on the current visit.
    pub visit_plays: u32,
    /// Ever complained about something, e.g. in chat.
    pub complained: bool,
    /// Whether the last connection was via an unreliable transport (e.g. WebTransport).
    pub last_unreliable: bool,
    /// Optionally, track a detailed log of events.
    pub quest: Option<ClientQuestData>,
}

/// Initializes from authenticate. Sets database fields to default values.
impl ClientMetricData {
    pub(crate) fn new(
        quest_fraction: f32,
        server_id: ServerId,
        arena_id: ArenaId,
        last_quest_date_created: &mut NonZeroUnixMillis,
        navigation: NavigationMetricsDto,
        lifecycle: LifecycleId,
    ) -> Self {
        let now = NonZeroUnixMillis::now();
        Self {
            cohort_id: thread_rng().gen(),
            language_id: Default::default(),
            user_agent_id: None,
            referrer: None,
            region_id: None,
            fps: None,
            rtt: None,
            date_created: now,
            lifecycle,
            invited: false,
            created: now,
            play_started: None,
            play_stopped: None,
            visit_started: None,
            visit_stopped: None,
            plays: 0,
            visit_plays: 0,
            complained: false,
            quest: (quest_fraction == 1.0 || thread_rng().gen::<f32>() < quest_fraction).then(
                || ClientQuestData::new(server_id, arena_id, navigation, last_quest_date_created),
            ),
            last_unreliable: false,
            timezone_offset: 0,
        }
    }

    pub(crate) fn update(&mut self, auth: &ClientAuthRequest) {
        let Self {
            cohort_id,
            language_id,
            timezone_offset,
            user_agent_id,
            region_id,
            fps: _,
            rtt: _,
            date_created,
            lifecycle: _,
            invited: _,
            referrer,
            created: _,
            complained: _,
            play_started: _,
            play_stopped: _,
            visit_started: _,
            visit_stopped: _,
            plays: _,
            visit_plays: _,
            quest: _,
            last_unreliable: _,
        } = self;
        *cohort_id = auth.cohort_id;
        *language_id = auth.language_id;
        *timezone_offset = auth.timezone_offset;
        *user_agent_id = auth.user_agent_id.or(*user_agent_id);
        *region_id = ip_to_region_id(auth.ip_address).or(*region_id);
        *referrer = auth.referrer.or(*referrer);
        *date_created = auth.date_created.min(*date_created);
    }
}

// TODO: was pub(crate)
/// Stores a T for each of several queries, and an aggregate.
#[derive(Default)]
pub struct Bundle<T> {
    pub(crate) total: T,
    pub(crate) by_cohort_id: HashMap<CohortId, T>,
    pub(crate) by_referrer: HashMap<Referrer, T>,
    pub(crate) by_region_id: HashMap<RegionId, T>,
    pub(crate) by_user_agent_id: HashMap<UserAgentId, T>,
    pub(crate) by_lifecycle: HashMap<LifecycleId, T>,
}

impl<T: Default> Bundle<T> {
    /// Visits a specific cross-section of the metrics.
    pub fn visit_specific_mut(
        &mut self,
        mut mutation: impl FnMut(&mut T),
        cohort_id: CohortId,
        referrer: Option<Referrer>,
        region_id: Option<RegionId>,
        user_agent_id: Option<UserAgentId>,
        lifecycle: LifecycleId,
        no_referrer: Option<Referrer>,
        other_referrer: Option<Referrer>,
        tracked_referrers: &HashMap<Referrer, Referrer>,
    ) {
        mutation(&mut self.total);
        mutation(self.by_cohort_id.entry(cohort_id).or_default());
        let referrer = if let Some(referrer) = referrer {
            tracked_referrers.get(&referrer).copied().or(other_referrer)
        } else {
            no_referrer
        };
        if let Some(referrer) = referrer {
            // We cap at the first few referrers we see to avoid unbounded memory.
            let referrers_full = self.by_referrer.len() >= 128;

            match self.by_referrer.entry(referrer) {
                Entry::Occupied(occupied) => mutation(occupied.into_mut()),
                Entry::Vacant(vacant) => {
                    if !referrers_full {
                        mutation(vacant.insert(T::default()))
                    }
                }
            }
        }
        if let Some(region_id) = region_id {
            mutation(self.by_region_id.entry(region_id).or_default());
        }
        if let Some(user_agent_id) = user_agent_id {
            mutation(self.by_user_agent_id.entry(user_agent_id).or_default());
        }
        mutation(self.by_lifecycle.entry(lifecycle).or_default());
    }

    /// Applies another bundle to this one, component-wise.
    pub fn apply<O>(&mut self, other: Bundle<O>, mut map: impl FnMut(&mut T, O)) {
        map(&mut self.total, other.total);
        for (cohort_id, o) in other.by_cohort_id {
            map(self.by_cohort_id.entry(cohort_id).or_default(), o);
        }
        for (referrer, o) in other.by_referrer {
            map(self.by_referrer.entry(referrer).or_default(), o);
        }
        for (region_id, o) in other.by_region_id {
            map(self.by_region_id.entry(region_id).or_default(), o);
        }
        for (user_agent_id, o) in other.by_user_agent_id {
            map(self.by_user_agent_id.entry(user_agent_id).or_default(), o);
        }
        for (lifecycle, o) in other.by_lifecycle {
            map(self.by_lifecycle.entry(lifecycle).or_default(), o);
        }
    }
}

impl<T: 'static> Bundle<T> {
    pub fn into_iter(self) -> impl Iterator<Item = (Option<MetricFilter>, T)> + 'static {
        iter::once((None, self.total))
            .chain(
                self.by_cohort_id
                    .into_iter()
                    .map(|(k, v)| (Some(MetricFilter::CohortId(k)), v)),
            )
            .chain(
                self.by_referrer
                    .into_iter()
                    .map(|(k, v)| (Some(MetricFilter::Referrer(k)), v)),
            )
            .chain(
                self.by_region_id
                    .into_iter()
                    .map(|(k, v)| (Some(MetricFilter::RegionId(k)), v)),
            )
            .chain(
                self.by_user_agent_id
                    .into_iter()
                    .map(|(k, v)| (Some(MetricFilter::UserAgentId(k)), v)),
            )
            .chain(
                self.by_lifecycle
                    .into_iter()
                    .map(|(k, v)| (Some(MetricFilter::LifecycleId(k)), v)),
            )
    }

    pub fn get(&self, filter: Option<MetricFilter>) -> Option<&T> {
        match filter {
            None => Some(&self.total),
            Some(MetricFilter::CohortId(cohort_id)) => self.by_cohort_id.get(&cohort_id),
            Some(MetricFilter::Referrer(referrer)) => self.by_referrer.get(&referrer),
            Some(MetricFilter::RegionId(region_id)) => self.by_region_id.get(&region_id),
            Some(MetricFilter::UserAgentId(user_agent_id)) => {
                self.by_user_agent_id.get(&user_agent_id)
            }
            Some(MetricFilter::LifecycleId(lifecycle)) => self.by_lifecycle.get(&lifecycle),
        }
    }
}

// TODO: was pub(crate)
/// Metrics total, and by various key types.
pub struct MetricBundle {
    pub(crate) start: NonZeroUnixMillis,
    pub(crate) bundle: Bundle<EngineMetrics>,
}

impl MetricBundle {
    pub fn new(start: NonZeroUnixMillis) -> Self {
        Self {
            start,
            bundle: Bundle::default(),
        }
    }

    pub fn metric(&self, filter: Option<MetricFilter>) -> EngineMetrics {
        self.bundle.get(filter).cloned().unwrap_or_default()
    }

    pub fn data_point(&self, filter: Option<MetricFilter>) -> EngineMetricsDataPointDto {
        self.bundle
            .get(filter)
            .map(|m| m.data_point())
            .unwrap_or_else(|| EngineMetrics::default().data_point())
    }
}

impl<G: ArenaService> MetricRepo<G> {
    // Speed up time to help debug.
    const MIN_VISIT_GAP: u64 = 30 * 60 * 1000;

    pub fn new() -> Self {
        let now = NonZeroUnixMillis::now();
        let current = MetricBundle::new(now.floor_hours());
        Self {
            no_referrer: None,
            other_referrer: None,
            tracked_referrers: Default::default(),
            startup: NonZeroUnixMillis::now(),
            next_swap: current.start.add_hours(1),
            next_update: now.floor_minutes().add_minutes(1),
            current,
            health: Default::default(),
            history: HistoryBuffer::default(),
            pending_quests: Default::default(),
            last_quest_date_created: NonZeroUnixMillis::MIN,
            _spooky: PhantomData,
        }
    }

    pub fn mutate_with(
        &mut self,
        mutation: impl Fn(&mut EngineMetrics),
        client_metric_data: &ClientMetricData,
    ) {
        self.current.bundle.visit_specific_mut(
            mutation,
            client_metric_data.cohort_id,
            client_metric_data.referrer,
            client_metric_data.region_id,
            client_metric_data.user_agent_id,
            client_metric_data.lifecycle,
            self.no_referrer,
            self.other_referrer,
            &self.tracked_referrers,
        );
    }

    /// Call when a websocket connects.
    pub fn start_visit(&mut self, client: &mut PlayerClientData<G>) {
        let renewed = client.metrics.lifecycle.is_renewed();

        debug_assert!(
            client.metrics.visit_started.is_none(),
            "visit already started"
        );
        client.metrics.visit_stopped = None;
        client.metrics.visit_started = Some(NonZeroUnixMillis::now());

        self.mutate_with(
            |m| {
                m.visits.increment();
                if renewed {
                    m.renews.increment();
                }
                // Here, we trust the client to send valid data. If it sent invalid an invalid
                // id, we will under-count new. However, we can't really stop the client from
                // forcing us to over-count new (by not sending a session despite having it).
                m.new.push(!renewed);
                // TODO: Don't count alternate domains, or softbear, as referrers.
                m.no_referrer.push(client.metrics.referrer.is_none());
            },
            &client.metrics,
        );
    }

    pub fn start_play(&mut self, player: &mut Player<G>) {
        let alias = player.alias;
        let score = player.liveboard.score.some().unwrap_or_default();
        let client = unwrap_or_return!(player.client_mut());

        debug_assert!(client.metrics.play_started.is_none(), "already started");

        let now = NonZeroUnixMillis::now();

        client.push_quest(QuestEvent::State {
            state: QuestState::Playing { alias, score },
        });

        if let Some(date_play_stopped) = client.metrics.play_stopped {
            let elapsed = now.millis_since(date_play_stopped);

            if elapsed > Self::MIN_VISIT_GAP {
                self.mutate_with(|m| m.visits.increment(), &client.metrics);
            }

            client.metrics.play_stopped = None;
        }

        client.metrics.play_started = Some(now);
        client.metrics.plays += 1;
        client.metrics.visit_plays += 1;
        self.mutate_with(|m| m.plays_total.increment(), &client.metrics)
    }

    pub fn stop_play(&mut self, player: &mut Player<G>) {
        let teamed = player.team_id.is_some();
        let client = unwrap_or_return!(player.client_mut());

        debug_assert!(client.metrics.play_stopped.is_none(), "already stopped");

        let now = NonZeroUnixMillis::now();

        client.with_quest(|quest| {
            quest.push(QuestEvent::State {
                state: if quest.quit {
                    QuestState::Spawning {}
                } else {
                    // TODO(quest): Death reason.
                    QuestState::Dead {
                        reason: "TODO".into(),
                    }
                },
            });
        });

        if let Some(play_started) = client.metrics.play_started {
            let elapsed = now.millis_since(play_started);
            let minutes_per_play = elapsed as f32 * (1.0 / 60.0 / 1000.0);
            self.mutate_with(
                |m| {
                    m.minutes_per_play.push(minutes_per_play);
                    m.minutes_per_play_histogram.push(minutes_per_play);
                    m.teamed.push(teamed);
                },
                &client.metrics,
            );

            client.metrics.play_started = None;
        } else {
            debug_assert!(false, "wasn't started");
        }

        client.metrics.play_stopped = Some(now);
    }

    pub fn stop_visit(&mut self, player: &mut Player<G>) {
        let mut client = unwrap_or_return!(player.client_mut());

        if self.pending_quests.len() < 100
            && let Some(quest) = client.metrics.quest.take()
        {
            let sample = quest.sample(&client.metrics);
            let mut string = serde_json::to_string_pretty(&sample).unwrap();
            string.push_str(",\n");
            async fn write_quest(string: String) -> std::io::Result<()> {
                use tokio::io::AsyncWriteExt;
                let mut file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("quests.log")
                    .await?;
                file.write_all(string.as_bytes()).await?;
                file.flush().await?;
                Ok(())
            }
            tokio::spawn(async move {
                if let Err(e) = write_quest(string).await {
                    log::error!("could not write quests log: {e}");
                } else {
                    log::info!("wrote quest log");
                }
            });
            self.pending_quests.push(sample);
        }

        if client.metrics.play_started.is_some() {
            debug_assert!(
                false,
                "technically valid, but play should have been stopped long ago"
            );
            self.stop_play(player);

            // Re-borrow.
            client = unwrap_or_return!(player.client_mut());
        }

        let now = NonZeroUnixMillis::now();

        let session_end = client
            .metrics
            .play_stopped
            .unwrap_or(client.metrics.created);
        let session_duration = session_end.millis_since(client.metrics.created);

        debug_assert!(client.metrics.visit_started.is_some());
        // Don't use `minutes_since` because that floors to integer.
        let minutes_per_visit = client
            .metrics
            .visit_started
            .map(|visit_started| now.millis_since(visit_started) as f32 * (1.0 / 60.0 / 1000.0));

        self.mutate_with(
            |m| {
                m.invited.push(client.metrics.invited);
                // Only consider unreliable if the client's last connection was unreliable,
                // meaning reliable fallback was probably not used.
                m.unreliable.push(client.metrics.last_unreliable);
                m.bounce.push(client.metrics.plays == 0);
                if client.metrics.plays > 0 {
                    m.unauthenticated.push(client.session.visitor_id.is_none());
                    if client.session.visitor_id.is_some() {
                        m.user.push(client.session.user);
                    }
                    m.complain.push(client.metrics.complained);
                    let peek_flop = client.metrics.plays == 1 && session_duration < 60 * 1000;
                    if client.metrics.lifecycle.is_new() {
                        // New player left promptly.
                        m.flop.push(peek_flop);
                    } else {
                        // Returning player left promptly.
                        m.peek.push(peek_flop);
                    }
                    if let Some(minutes_per_visit) = minutes_per_visit {
                        m.minutes_per_visit.push(minutes_per_visit);
                        m.minutes_per_visit_histogram.push(minutes_per_visit);
                    }
                    m.plays_per_visit.push(client.metrics.visit_plays as f32);
                    m.plays_per_visit_histogram
                        .push(client.metrics.visit_plays as f32);
                }
            },
            &client.metrics,
        );

        client.metrics.visit_started = None;
        client.metrics.visit_stopped = Some(now);
        client.metrics.visit_plays = 0;
    }

    /// Returns metric to safe in database, if any.
    fn update(
        infrastructure: &mut ServerActor<G>,
    ) -> Option<(NonZeroUnixMillis, Bundle<EngineMetrics>)> {
        let metrics_repo = &mut infrastructure.metrics;

        let now = NonZeroUnixMillis::now();

        if now < metrics_repo.next_update {
            return None;
        }
        metrics_repo.next_update = now.floor_minutes().add_minutes(1);

        // Uptime in seconds.
        let uptime = now.millis_since(metrics_repo.startup) / 1000;
        for (_, tier) in infrastructure.realms.iter_mut() {
            let context = &mut tier.arena.arena_context;
            metrics_repo
                .current
                .bundle
                .total
                .world_size
                .push(tier.arena.arena_service.world_size());
            metrics_repo
                .current
                .bundle
                .total
                .entities
                .push(tier.arena.arena_service.entities() as f32);

            let mut concurrent = Bundle::<u32>::default();

            for (_, player) in context.players.iter() {
                if !player.is_alive() {
                    continue;
                }
                if let Some(client) = player.client() {
                    concurrent.visit_specific_mut(
                        |c| *c += 1,
                        client.metrics.cohort_id,
                        client.metrics.referrer,
                        client.metrics.region_id,
                        client.metrics.user_agent_id,
                        client.metrics.lifecycle,
                        metrics_repo.no_referrer,
                        metrics_repo.other_referrer,
                        &metrics_repo.tracked_referrers,
                    );
                    metrics_repo.mutate_with(
                        |m| {
                            if let Some(fps) = client.metrics.fps {
                                m.fps.push(fps);
                                m.low_fps.push(fps < 24.0);
                            }
                            if let Some(rtt) = client.metrics.rtt {
                                m.rtt.push(rtt as f32 * 0.001);
                            }
                            if let Score::Some(score) = player.liveboard.score {
                                m.score.push(score as f32);
                            }

                            let retention_millis = now.millis_since(client.metrics.date_created);
                            let retention = (retention_millis as f64
                                * (1.0 / NonZeroUnixMillis::MILLIS_PER_DAY as f64))
                                as f32;
                            m.retention_days.push(retention);
                            m.retention_histogram.push(retention);
                        },
                        &client.metrics,
                    );
                }
            }

            metrics_repo
                .current
                .bundle
                .apply(concurrent, |metrics, concurrent| {
                    if concurrent > 0 {
                        metrics.concurrent.push(concurrent as f32)
                    }
                });
        }

        let health = &mut metrics_repo.health;
        let mut general = |m: &mut EngineMetrics| {
            m.cpu.push(health.cpu());
            m.cpu_steal.push(health.cpu_steal());
            m.ram.push(health.ram());
            const MEGABIT: f32 = 125000.0;
            m.bandwidth_rx.push(health.bandwidth_rx() as f32 / MEGABIT);
            m.bandwidth_tx.push(health.bandwidth_tx() as f32 / MEGABIT);
            let mut connections = 0;
            {
                let lim = HTTP_RATE_LIMITER.lock().unwrap();
                for (c, a) in lim.connections_actives_per_ip() {
                    connections += c as usize;
                    m.connections_per_ip_histogram.push(c as f32);
                    m.actives_per_ip_histogram.push(a as f32);
                }
            }
            m.connections.push(connections as f32);
            m.conntracks.push(health.connections() as f32);
            m.tasks
                .push(Handle::current().metrics().num_alive_tasks() as f32);
            m.tps = m.tps + health.take_tps();
            m.spt = m.spt + health.take_spt();
            m.uptime.push(uptime as f32 / (24.0 * 60.0 * 60.0));
        };
        // metrics_repo.mutate_all(general);
        general(&mut metrics_repo.current.bundle.total);

        if now < metrics_repo.next_swap {
            return None;
        }
        let new_current = now.floor_hours();
        metrics_repo.next_swap = new_current.add_hours(1);

        let mut current = MetricBundle::new(metrics_repo.current.start);
        current.bundle.total = Self::get_metrics(infrastructure, None);

        macro_rules! copy {
            ($infrastructure: expr, $new: expr, $map: ident, $variant: ident) => {
                for key in $infrastructure
                    .metrics
                    .current
                    .bundle
                    .$map
                    .keys()
                    .copied()
                    .collect::<Vec<_>>()
                    .into_iter()
                {
                    $new.bundle.$map.insert(
                        key,
                        Self::get_metrics($infrastructure, Some(MetricFilter::$variant(key))),
                    );
                }
            };
        }

        copy!(infrastructure, current, by_cohort_id, CohortId);
        copy!(infrastructure, current, by_user_agent_id, UserAgentId);
        copy!(infrastructure, current, by_referrer, Referrer);
        copy!(infrastructure, current, by_region_id, RegionId);
        copy!(infrastructure, current, by_lifecycle, LifecycleId);

        macro_rules! collect {
            ($map: ident) => {
                collect!($map, |_| true)
            };
            ($map: ident, $filter: expr) => {{
                current
                    .bundle
                    .$map
                    .iter()
                    .filter_map(|(&key, m)| $filter(key).then(|| (key, m.clone())))
                    .collect()
            }};
        }

        let save_to_db = Bundle {
            total: current.bundle.total.clone(),
            by_cohort_id: collect!(by_cohort_id),
            by_referrer: collect!(by_referrer),
            by_region_id: collect!(by_region_id),
            by_user_agent_id: collect!(by_user_agent_id),
            by_lifecycle: collect!(by_lifecycle),
        };

        let timestamp = current.start;

        infrastructure.metrics.history.write(current);
        infrastructure.metrics.current = MetricBundle::new(new_current);

        Some((timestamp, save_to_db))
    }

    pub fn update_to_plasma(
        infrastructure: &mut ServerActor<G>,
        ctx: &mut ActorContext<ServerActor<G>>,
    ) {
        if let Some((timestamp, bundle)) = Self::update(infrastructure) {
            let server_id = infrastructure.server_id;
            // Don't hammer the database row from multiple servers simultaneously, which
            // wouldn't compromise correctness, but would affect performance (number of retries).
            tokio::time::sleep(Duration::from_secs(if server_id.kind.is_cloud() {
                server_id.number.0.get() as u64 * 5
            } else {
                server_id.number.0.get() as u64 % 5
            }))
            .into_actor(infrastructure)
            .map(move |_, act, _ctx| {
                act.plasma.do_request(PlasmaRequestV1::UpdateMetrics {
                    timestamp,
                    metrics: bundle.into_iter().collect(),
                });
            })
            .spawn(ctx)
        }
    }

    pub fn get_metrics(
        infrastructure: &mut ServerActor<G>,
        filter: Option<MetricFilter>,
    ) -> EngineMetrics {
        // Get basis.
        let metrics_repo = &mut infrastructure.metrics;
        let mut metrics = metrics_repo
            .current
            .bundle
            .get(filter)
            .cloned()
            .unwrap_or_default();

        for (_, tier) in infrastructure.realms.iter() {
            // NOTE: The database compare and swap relies on it changing.
            metrics.arenas_cached.increment();

            // But these don't matter for the compare and swap and do not pertain to individual filters.
            if filter.is_none() {
                metrics
                    .players_cached
                    .add_length(tier.arena.arena_context.players.len());
                metrics
                    .sessions_cached
                    .add_length(tier.arena.arena_context.players.real_players as usize);
            }
        }

        if filter.is_none() {
            metrics
                .invitations_cached
                .add_length(infrastructure.invitations.len());
        }

        metrics
    }
}
