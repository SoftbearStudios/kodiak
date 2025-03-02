// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{
    ContinuousExtremaMetricAccumulator, DiscreteMetricAccumulator, DistinctCountMetricAccumulator,
    DistinctCountMetricSummary, HistogramMetricAccumulator, RatioMetricAccumulator,
};
use crate::{is_default, CohortId, LifecycleId, Referrer, RegionId, UserAgentId};
use derive_more::Add;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::iter::Sum;
use std::ops::Add;

pub trait MetricAccumulator: Sized + Add + Default {
    type Summary: Serialize + DeserializeOwned;

    // Must be a tuple. First value is most important.
    type DataPoint: Serialize + DeserializeOwned;

    fn summarize(&self) -> Self::Summary;
    fn data_point(&self) -> Self::DataPoint;
}

/// The Metrics Data Transfer Object (DTO) contains core server metrics.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct MetricsSummaryDto {
    pub abuse_reports: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub actives_per_ip_histogram: <HistogramMetricAccumulator<10> as MetricAccumulator>::Summary,
    pub alt_domain: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub arenas_cached: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub bandwidth_rx: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub bandwidth_tx: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub banner_ads: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub bounce: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub chats: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub complain: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub concurrent: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub connections: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub connections_per_ip_histogram:
        <HistogramMetricAccumulator<20> as MetricAccumulator>::Summary,
    pub conntracks: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub cpu: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub cpu_steal: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub crashes: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub dns: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub dom: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub entities: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub flop: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub fps: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub http: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub invited: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub invitations_cached: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub low_fps: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub minutes_per_play: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub minutes_per_play_histogram: <HistogramMetricAccumulator<10> as MetricAccumulator>::Summary,
    pub minutes_per_visit: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub minutes_per_visit_histogram: <HistogramMetricAccumulator<30> as MetricAccumulator>::Summary,
    pub new: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub no_referrer: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub peek: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub players_cached: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub plays_per_visit: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub plays_per_visit_histogram: <HistogramMetricAccumulator<10> as MetricAccumulator>::Summary,
    pub plays_total: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub ram: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub renews: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub retention_days: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub retention_histogram: <HistogramMetricAccumulator<10> as MetricAccumulator>::Summary,
    pub rewarded_ads: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub rtt: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub score: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub sessions_cached: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub spt: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub tasks: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub tcp: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub teamed: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub tls: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub toxicity: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub tps: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub unauthenticated: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub unreliable: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub uptime: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
    pub user: <RatioMetricAccumulator as MetricAccumulator>::Summary,
    pub video_ads: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub visitors: DistinctCountMetricSummary,
    pub visits: <DiscreteMetricAccumulator as MetricAccumulator>::Summary,
    pub world_size: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::Summary,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct EngineMetricsDataPointDto {
    pub abuse_reports: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub actives_per_ip_histogram: <HistogramMetricAccumulator<10> as MetricAccumulator>::DataPoint,
    pub alt_domain: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub arenas_cached: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub bandwidth_rx: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub bandwidth_tx: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub banner_ads: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub bounce: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub chats: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub complain: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub concurrent: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub connections: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub connections_per_ip_histogram:
        <HistogramMetricAccumulator<20> as MetricAccumulator>::DataPoint,
    pub conntracks: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub cpu: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub cpu_steal: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub crashes: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub dns: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub dom: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub entities: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub flop: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub fps: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub http: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub invited: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub invitations_cached: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub low_fps: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub minutes_per_play: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub minutes_per_play_histogram:
        <HistogramMetricAccumulator<10> as MetricAccumulator>::DataPoint,
    pub minutes_per_visit: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub minutes_per_visit_histogram:
        <HistogramMetricAccumulator<30> as MetricAccumulator>::DataPoint,
    pub new: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub no_referrer: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub peek: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub players_cached: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub plays_per_visit: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub plays_per_visit_histogram: <HistogramMetricAccumulator<10> as MetricAccumulator>::DataPoint,
    pub plays_total: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub ram: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub renews: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub retention_days: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub retention_histogram: <HistogramMetricAccumulator<10> as MetricAccumulator>::DataPoint,
    pub rewarded_ads: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub rtt: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub score: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub sessions_cached: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub spt: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub tasks: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub tcp: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub teamed: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub tls: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub toxicity: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub tps: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub unauthenticated: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub unreliable: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub uptime: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
    pub user: <RatioMetricAccumulator as MetricAccumulator>::DataPoint,
    pub video_ads: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub visitors: (u32,),
    pub visits: <DiscreteMetricAccumulator as MetricAccumulator>::DataPoint,
    pub world_size: <ContinuousExtremaMetricAccumulator as MetricAccumulator>::DataPoint,
}

/// Filter metrics.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum MetricFilter {
    CohortId(CohortId),
    LifecycleId(LifecycleId),
    Referrer(Referrer),
    RegionId(RegionId),
    UserAgentId(UserAgentId),
}

#[derive(Clone, Debug, Default, Add, Deserialize, Serialize)]
pub struct EngineMetrics {
    /// Number of active abuse reports.
    #[serde(default, skip_serializing_if = "is_default")]
    pub abuse_reports: DiscreteMetricAccumulator,
    /// How many active clients on the game server process were permitted, per IP.
    #[serde(default, skip_serializing_if = "is_default")]
    pub actives_per_ip_histogram: HistogramMetricAccumulator<10>,
    /// Ratio of visitors via an alternative domain.
    #[serde(default, skip_serializing_if = "is_default")]
    pub alt_domain: RatioMetricAccumulator,
    /// How many arenas are in cache.
    #[serde(default, skip_serializing_if = "is_default")]
    pub arenas_cached: DiscreteMetricAccumulator,
    /// How many megabits per second received.
    #[serde(default, skip_serializing_if = "is_default")]
    pub bandwidth_rx: ContinuousExtremaMetricAccumulator,
    /// How many megabits per second transmitted.
    #[serde(default, skip_serializing_if = "is_default")]
    pub bandwidth_tx: ContinuousExtremaMetricAccumulator,
    /// Number of banner advertisements shown.
    #[serde(default, skip_serializing_if = "is_default")]
    pub banner_ads: DiscreteMetricAccumulator,
    /// Ratio of new players that leave without ever playing.
    #[serde(default, skip_serializing_if = "is_default")]
    pub bounce: RatioMetricAccumulator,
    /// Number of chat messages sent.
    #[serde(default, skip_serializing_if = "is_default")]
    pub chats: DiscreteMetricAccumulator,
    /// Ratio of players who complained in chat.
    #[serde(default, skip_serializing_if = "is_default")]
    pub complain: RatioMetricAccumulator,
    /// How many concurrent players.
    #[serde(default, skip_serializing_if = "is_default")]
    pub concurrent: ContinuousExtremaMetricAccumulator,
    /// How many TCP/UDP connections to the game server process were permitted.
    #[serde(default, skip_serializing_if = "is_default")]
    pub connections: ContinuousExtremaMetricAccumulator,
    /// How many TCP/UDP connections to the game server process were permitted, per IP.
    #[serde(default, skip_serializing_if = "is_default")]
    pub connections_per_ip_histogram: HistogramMetricAccumulator<20>,
    /// How many connections are tracked by conntrack.
    #[serde(default, skip_serializing_if = "is_default")]
    pub conntracks: ContinuousExtremaMetricAccumulator,
    /// Fraction of total CPU time used by processes in the current operating system.
    #[serde(default, skip_serializing_if = "is_default")]
    pub cpu: ContinuousExtremaMetricAccumulator,
    /// Fraction of total CPU time stolen by the hypervisor.
    #[serde(default, skip_serializing_if = "is_default")]
    pub cpu_steal: ContinuousExtremaMetricAccumulator,
    /// Client crashes.
    #[serde(default, skip_serializing_if = "is_default")]
    pub crashes: DiscreteMetricAccumulator,
    /// Milliseconds taken by DNS lookup.
    ///
    /// In `PerformanceNavigationTiming` terms, this is `domainLookupEnd` - `domainLookupStart`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dns: ContinuousExtremaMetricAccumulator,
    /// Milliseconds from browser DOM loading start to finish.
    ///
    /// In `PerformanceNavigationTiming` terms, this is `loadEventEnd` - `domInteractive`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dom: ContinuousExtremaMetricAccumulator,
    /// Ratio of new players that play only once and leave quickly.
    #[serde(default, skip_serializing_if = "is_default")]
    pub flop: RatioMetricAccumulator,
    /// Client frames per second.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fps: ContinuousExtremaMetricAccumulator,
    /// Milliseconds for initial HTTP request and response.
    ///
    /// In `PerformanceNavigationTiming` terms, this is `responseEnd` - `requestStart`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub http: ContinuousExtremaMetricAccumulator,
    /// Ratio of new players who were invited to new players who were not.
    #[serde(default, skip_serializing_if = "is_default")]
    pub invited: RatioMetricAccumulator,
    /// Number of invitations in RAM cache.
    #[serde(default, skip_serializing_if = "is_default")]
    pub invitations_cached: DiscreteMetricAccumulator,
    /// Ratio of players with FPS below 24 to all players.
    #[serde(default, skip_serializing_if = "is_default")]
    pub low_fps: RatioMetricAccumulator,
    /// Minutes per completed play (a measure of engagement).
    #[serde(default, skip_serializing_if = "is_default")]
    pub minutes_per_play: ContinuousExtremaMetricAccumulator,
    /// Minutes per play histogram.
    #[serde(default, skip_serializing_if = "is_default")]
    pub minutes_per_play_histogram: HistogramMetricAccumulator<10>,
    /// Minutes played, per visit, during the metrics period.
    #[serde(default, skip_serializing_if = "is_default")]
    pub minutes_per_visit: ContinuousExtremaMetricAccumulator,
    /// Minutes per visit histogram.
    #[serde(default, skip_serializing_if = "is_default")]
    pub minutes_per_visit_histogram: HistogramMetricAccumulator<30>,
    /// Ratio of unique players that are new to players that are not.
    #[serde(default, skip_serializing_if = "is_default")]
    pub new: RatioMetricAccumulator,
    /// Ratio of players with no referrer to all players.
    #[serde(default)]
    pub no_referrer: RatioMetricAccumulator,
    /// Ratio of previous players that leave without playing (e.g. to peek at player count).
    #[serde(default, skip_serializing_if = "is_default")]
    pub peek: RatioMetricAccumulator,
    /// How many players (for now, [`PlayerId`]) are in memory cache.
    #[serde(default, skip_serializing_if = "is_default")]
    pub players_cached: DiscreteMetricAccumulator,
    /// Plays per visit (a measure of engagement).
    #[serde(default, skip_serializing_if = "is_default")]
    pub plays_per_visit: ContinuousExtremaMetricAccumulator,
    /// Plays per visit histogram.
    #[serde(default, skip_serializing_if = "is_default")]
    pub plays_per_visit_histogram: HistogramMetricAccumulator<10>,
    /// Plays total (aka impressions).
    #[serde(default, skip_serializing_if = "is_default")]
    pub plays_total: DiscreteMetricAccumulator,
    /// Percent of available server RAM required by service.
    #[serde(default, skip_serializing_if = "is_default")]
    pub ram: ContinuousExtremaMetricAccumulator,
    /// Number of times session was renewed.
    #[serde(default, skip_serializing_if = "is_default")]
    pub renews: DiscreteMetricAccumulator,
    /// Player retention in days.
    #[serde(default, skip_serializing_if = "is_default")]
    pub retention_days: ContinuousExtremaMetricAccumulator,
    /// Player retention histogram.
    #[serde(default, skip_serializing_if = "is_default")]
    pub retention_histogram: HistogramMetricAccumulator<10>,
    /// Number of rewarded advertisements shown.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rewarded_ads: DiscreteMetricAccumulator,
    /// Network latency round trip time in seconds.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rtt: ContinuousExtremaMetricAccumulator,
    /// Score per completed play.
    #[serde(default, skip_serializing_if = "is_default")]
    pub score: ContinuousExtremaMetricAccumulator,
    /// Total sessions in cache.
    #[serde(default, skip_serializing_if = "is_default")]
    pub sessions_cached: DiscreteMetricAccumulator,
    /// Seconds per tick.
    #[serde(default, skip_serializing_if = "is_default")]
    pub spt: ContinuousExtremaMetricAccumulator,
    /// How many async runtime tasks are active.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tasks: ContinuousExtremaMetricAccumulator,
    /// Milliseconds to establish a TCP connection.
    ///
    /// In `PerformanceNavigationTiming` terms, this is min(`connnectEnd`, `secureConnectionStart`) - `connectStart`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tcp: ContinuousExtremaMetricAccumulator,
    /// Ratio of plays that end team-less to plays that don't.
    #[serde(default, skip_serializing_if = "is_default")]
    pub teamed: RatioMetricAccumulator,
    /// Milliseconds to establish TLS.
    ///
    /// In `PerformanceNavigationTiming` terms, this is `connectEnd` - `secureConnectionStart`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tls: ContinuousExtremaMetricAccumulator,
    /// Ratio of inappropriate messages to total.
    #[serde(default, skip_serializing_if = "is_default")]
    pub toxicity: RatioMetricAccumulator,
    /// Server ticks per second.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tps: ContinuousExtremaMetricAccumulator,
    /// Ratio of visitors who could not be authenticated with plasma.
    #[serde(default, skip_serializing_if = "is_default")]
    pub unauthenticated: RatioMetricAccumulator,
    /// Ratio of clients using an unreliable network transport (e.g. WebTransport).
    #[serde(default, skip_serializing_if = "is_default")]
    pub unreliable: RatioMetricAccumulator,
    /// Uptime in (fractional) days.
    #[serde(default, skip_serializing_if = "is_default")]
    pub uptime: ContinuousExtremaMetricAccumulator,
    /// Ratio of authenticated visitors who are also users.
    #[serde(default, skip_serializing_if = "is_default")]
    pub user: RatioMetricAccumulator,
    /// Number of video advertisements shown.
    #[serde(default, skip_serializing_if = "is_default")]
    pub video_ads: DiscreteMetricAccumulator,
    /// Unique visitors.
    #[serde(default, skip_serializing_if = "is_default")]
    pub visitors: DistinctCountMetricAccumulator<[u8; 1024]>,
    /// Visits
    #[serde(default, skip_serializing_if = "is_default")]
    pub visits: DiscreteMetricAccumulator,
    #[serde(default, skip_serializing_if = "is_default")]
    pub entities: ContinuousExtremaMetricAccumulator,
    #[serde(default, skip_serializing_if = "is_default")]
    pub world_size: ContinuousExtremaMetricAccumulator,
}

#[macro_export]
macro_rules! fields {
    ($me: ident, $st: ident, $f: ident, $($name: ident,)*) => {
        {
            $st {
                $($name: $me.$name.$f()),*
            }
        }
    }
}

impl EngineMetrics {
    pub fn summarize(&self) -> MetricsSummaryDto {
        fields!(
            self,
            MetricsSummaryDto,
            summarize,
            // Fields
            abuse_reports,
            actives_per_ip_histogram,
            alt_domain,
            arenas_cached,
            bandwidth_rx,
            bandwidth_tx,
            banner_ads,
            bounce,
            chats,
            complain,
            concurrent,
            connections,
            connections_per_ip_histogram,
            conntracks,
            cpu,
            cpu_steal,
            crashes,
            dns,
            dom,
            entities,
            flop,
            fps,
            http,
            invited,
            invitations_cached,
            low_fps,
            minutes_per_play,
            minutes_per_play_histogram,
            minutes_per_visit,
            minutes_per_visit_histogram,
            new,
            no_referrer,
            peek,
            players_cached,
            plays_per_visit,
            plays_per_visit_histogram,
            plays_total,
            ram,
            renews,
            retention_days,
            retention_histogram,
            rewarded_ads,
            rtt,
            score,
            sessions_cached,
            spt,
            tasks,
            tcp,
            teamed,
            tls,
            toxicity,
            tps,
            unauthenticated,
            unreliable,
            uptime,
            user,
            video_ads,
            visitors,
            visits,
            world_size,
        )
    }

    pub fn data_point(&self) -> EngineMetricsDataPointDto {
        fields! {
            self,
            EngineMetricsDataPointDto,
            data_point,
            // Fields.
            abuse_reports,
            actives_per_ip_histogram,
            alt_domain,
            arenas_cached,
            bandwidth_rx,
            bandwidth_tx,
            banner_ads,
            bounce,
            chats,
            complain,
            concurrent,
            connections,
            connections_per_ip_histogram,
            conntracks,
            cpu,
            cpu_steal,
            crashes,
            dns,
            dom,
            entities,
            flop,
            fps,
            http,
            invited,
            invitations_cached,
            low_fps,
            minutes_per_play,
            minutes_per_play_histogram,
            minutes_per_visit,
            minutes_per_visit_histogram,
            new,
            no_referrer,
            peek,
            players_cached,
            plays_per_visit,
            plays_per_visit_histogram,
            plays_total,
            ram,
            renews,
            retention_days,
            retention_histogram,
            rewarded_ads,
            rtt,
            score,
            sessions_cached,
            spt,
            tasks,
            tcp,
            teamed,
            tls,
            toxicity,
            tps,
            unauthenticated,
            unreliable,
            uptime,
            user,
            video_ads,
            visitors,
            visits,
            world_size,
        }
    }
}

impl Sum for EngineMetrics {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut total = Self::default();
        for item in iter {
            total = total + item;
        }
        total
    }
}
