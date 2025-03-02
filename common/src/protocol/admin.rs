// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    ClientHash, EngineMetricsDataPointDto, GameId, MetricFilter, MetricsSummaryDto,
    NonZeroUnixMillis, Owned, PlayerAlias, PlayerId, Referrer, RegionId, ServerId, ServerNumber,
    SessionToken, TeamId, UserAgentId,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::net::IpAddr;

/// Admin requests are from the admin interface to the core service.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "server", derive(actix::Message))]
#[cfg_attr(
    feature = "server",
    rtype(result = "Result<AdminUpdate, &'static str>")
)]
pub enum AdminRequest {
    OverridePlayerAlias {
        player_id: PlayerId,
        alias: PlayerAlias,
    },
    OverridePlayerModerator {
        player_id: PlayerId,
        moderator: bool,
    },
    RequestDay {
        filter: Option<MetricFilter>,
    },
    RequestGames,
    RequestPlayers,
    /// Request an n-second CPU profile.
    RequestCpuProfile(u16),
    /// Request an n-second heap profile.
    RequestHeapProfile(u16),
    RequestReferrers,
    RequestRegions,
    RequestSeries {
        game_id: GameId,
        server_id: Option<ServerId>,
        filter: Option<MetricFilter>,
        period_start: Option<NonZeroUnixMillis>,
        period_stop: Option<NonZeroUnixMillis>,
        // Resolution in hours.
        resolution: Option<std::num::NonZeroU8>,
    },
    /// Qualifies the result of RequestDay and RequestSummary.
    RequestServerId,
    RequestSummary {
        filter: Option<MetricFilter>,
    },
    RequestUserAgents,
}

/// Admin related responses from the server.
#[derive(Clone, Debug, Serialize)]
pub enum AdminUpdate {
    ChatSent,
    DayRequested(Owned<[(NonZeroUnixMillis, EngineMetricsDataPointDto)]>),
    GamesRequested(Box<[(GameId, f32)]>),
    HttpServerRestarting,
    PlayerAliasOverridden(PlayerAlias),
    PlayerModeratorOverridden(bool),
    PlayerMuted(usize),
    PlayersRequested(Box<[AdminPlayerDto]>),
    CpuProfileRequested(String),
    HeapProfileRequested(String),
    ReferrersRequested(Box<[(Referrer, f32)]>),
    RegionsRequested(Box<[(RegionId, f32)]>),
    SeriesRequested(Owned<[(NonZeroUnixMillis, EngineMetricsDataPointDto)]>),
    ServerIdRequested(ServerId),
    SummaryRequested(Box<MetricsSummaryDto>),
    UserAgentsRequested(Box<[(UserAgentId, f32)]>),
}

/// The Player Admin Data Transfer Object (DTO) binds player ID to admin player data (for real players, not bots).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AdminPlayerDto {
    pub alias: PlayerAlias,
    pub player_id: PlayerId,
    pub team_id: Option<TeamId>,
    pub region_id: Option<RegionId>,
    pub session_token: Option<SessionToken>,
    pub ip_address: IpAddr,
    pub moderator: bool,
    pub score: u32,
    pub plays: u32,
    pub fps: Option<f32>,
    pub rtt: Option<u16>,
}

/// Deprecated. Like [`InstancePickerDto`] but more details.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdminServerDto {
    pub server_number: ServerNumber,
    pub redirect_server_number: Option<ServerNumber>,
    pub region_id: RegionId,
    pub ip: IpAddr,
    /// Routed by DNS to the home page.
    pub home: bool,
    pub healthy: bool,
    pub client_hash: ClientHash,
    pub player_count: u32,
}

impl PartialOrd for AdminServerDto {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AdminServerDto {
    fn cmp(&self, other: &Self) -> Ordering {
        self.server_number.cmp(&other.server_number)
    }
}
