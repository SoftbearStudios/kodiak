// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{ChatRecipient, ClaimUpdateDto, FileNamespace, RealmHeartbeat, ServerLogDto};
use crate::{
    is_default, ArenaId, ArenaToken, ChatId, ClientHash, EngineMetrics, GameId,
    LeaderboardScoreDto, MetricFilter, NonZeroUnixMillis, PlayerAlias, PlayerId, QuestSampleDto,
    RealmId, ServerId, SessionToken, TeamName, TeamToken, VisitorId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::net::IpAddr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaDeveloper {
    V1(PlasmaDeveloperV1),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaDeveloperV1 {
    /// Register game.
    RegisterGame {
        /// The game domain.  For example, "foobar.com".
        domain: String,
        /// The game ID.  For example, "FooBar".
        game_id: GameId,
        /// The game name.  For example, "Foo Bar" or "FooBar.com".
        game_name: String,
        /// Whether the game wants geodns to route players to the closest server right away.
        geodns_enabled: bool,
        /// The game trademark.  For example, "Foo Bar".
        trademark: String,
        /// Whether the game wants UDP allowed by the firewall.
        udp_enabled: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaRequest {
    V1(PlasmaRequestV1),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaRequestV1 {
    /// Authenticates player as follows: if player is signed in, Plasma sends
    /// [`Player`] in response to return the player's `visitor_id`, claims, etc.
    /// If player is not signed in, Plasma sends [`Claims`] to return claims.
    AuthenticatePlayer {
        /// Navigational ID which includes realm, tier, etc.
        #[serde(default)]
        arena_id: ArenaId,
        /// Uniquely identifies the arena, for idempotency.
        arena_token: ArenaToken,
        /// Latest player ID, replaces any previous player ID of this user in this arena.
        player_id: PlayerId,
        /// Uniquely identifies player for short term, but without the access privileges of session ID.
        session_token: SessionToken,
    },
    /// Sent every 60 seconds. Server considered dead if not received in last 180s.
    Heartbeat {
        /// Claims may be piggy back on heartbeat.
        #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
        claims: Box<[ClaimUpdateDto]>,
        /// Hash of client binary, in case it changed.
        client_hash: ClientHash,
        /// CPU utilization from 0 to 1.
        #[serde(default, skip_serializing_if = "is_default")]
        cpu: f32,
        /// RAM utilization from 0 to 1.
        #[serde(default, skip_serializing_if = "is_default")]
        ram: f32,
        /// Fraction of game ticks missed from 0 to 1.
        #[serde(default, skip_serializing_if = "is_default")]
        missed_ticks: f32,
        /// The time, in milliseconds since the Unix epoch, that the
        /// primary certificate will expire. Any additional certificates
        /// are not reported.
        ///
        /// `None` will be sent by old servers, `Some(past)` may be sent
        /// if the certificate is missing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date_certificate_expires: Option<NonZeroUnixMillis>,
        /// Heartbeats for each arena on the server.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        realms: BTreeMap<RealmId, RealmHeartbeat>,
    },
    /// Load binary data from a file for `visitor_id`.
    LoadFile {
        /// Where to find the file.
        file_namespace: FileNamespace,
        /// Path of the file that was uploaded to plasma.
        file_path: String,
        /// Only send the file if it has this content type. If `None`, allow any content type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        accept_content_type: Option<String>,
        /// Visitor ID of the player that is downloading the data, or `None` if their
        /// authentication failed. For permission checking and echoing in the response.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visitor_id: Option<VisitorId>,
        /// Echo in response.
        arena_id: ArenaId,
        /// Echo in response.
        player_id: PlayerId,
    },
    /// Moderates a specific chat message. The immediate consequence may depend
    /// upon whether `user_id` is an authorized moderator because, generally,
    /// the more severe consequences require authorization. The minimum consequence
    /// is to "accuse" the chat message of being abusive.
    ///
    ModerateAbuse {
        /// Alias of the reporter/moderator.
        #[serde(default, skip_serializing_if = "is_default")]
        alias: PlayerAlias,
        /// ID of the offensive chat message.
        chat_id: ChatId,
        /// Visitor ID of the reporter/moderator.
        visitor_id: VisitorId,
    },
    /// Set (or overwrite) chat policy for the specified realm.
    ModerateChat {
        #[serde(default, skip_serializing_if = "is_default")]
        /// Alias of the moderator.
        alias: PlayerAlias,
        /// Arena ID of the realm whose chat is to be moderated.
        #[serde(default, skip_serializing_if = "is_default")]
        arena_id: ArenaId,
        /// Implement "safe mode" for the next X minutes. If `None`, has no effect.
        #[serde(default, skip_serializing_if = "is_default")]
        safe_mode: Option<u32>,
        /// Implement "slow mode" for the next X minutes. If `None`, has no effect.
        #[serde(default, skip_serializing_if = "is_default")]
        slow_mode: Option<u32>,
        /// Visitor ID of the moderator.
        visitor_id: VisitorId,
    },
    /// A server has started running.  Registers the [`game_id`] and [`server_id`]
    /// provided when opening web socket.
    ///
    /// Plasma sends [`Referrers`] and [`Snippets`] in response.
    //
    // {} is for backward compatibility
    RegisterServer {
        // TODO: this is an Option for backward compatibility but eventually won't be an Option.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date_started: Option<NonZeroUnixMillis>,
    },
    /// Releases team name.  No response is sent.
    ReleaseTeamName {
        /// Arena ID of releasor.
        #[serde(default, skip_serializing_if = "is_default")]
        arena_id: ArenaId,
        /// Player ID of releasor.
        player_id: PlayerId,
        /// Team name, for validation.
        team_name: TeamName,
        /// Proof that team was reserved.
        team_token: TeamToken,
    },
    /// Plasma sends [`TeamName`] in response if the team name is available.
    ReserveTeamName {
        /// Arena ID of requestor (scene id may change as player moves
        /// between arenas, but realm id remains constant).
        #[serde(default, skip_serializing_if = "is_default")]
        arena_id: ArenaId,
        /// When the reservation expires, unless it expires in one hour. (If the
        /// date/time in the past, e.g. 0, it is equivalent to `ReleaseTeamName`.)
        expires: Option<NonZeroUnixMillis>,
        /// Player ID of requestor (may change as player moves between arenas).
        player_id: PlayerId,
        /// Unique within a realm.
        team_name: TeamName,
        /// For renewing team. This is constant for the duration team is reserved.
        team_token: Option<TeamToken>,
    },
    /// Save binary data for `visitor_id` as a shared file with no teaser.
    SaveFile {
        /// Content (data) of the file that is uploaded to plasma.
        content_data: Vec<u8>,
        /// MIME type of the content in the file that is uploaded to plasma.
        content_type: Option<String>,
        /// Path of the file that is uploaded to plasma.
        file_path: String,
        /// Visitor ID of the player that is uploading the data.
        visitor_id: VisitorId,
        /// Echo in response.
        arena_id: ArenaId,
        /// Echo in response.
        player_id: PlayerId,
    },
    /// Send chat message for distribution to other servers.
    SendChat {
        /// Flag that chat was sent by server itself (shouldn't be sent to any servers, only logged?).
        #[serde(default, skip_serializing_if = "is_default")]
        admin: bool,
        /// Alias of the sender.
        alias: PlayerAlias,
        /// Navigational ID which includes realm, tier, etc.
        #[serde(default)]
        arena_id: ArenaId,
        /// Whether the sender is authentic (nickname matches alias).
        #[serde(default, skip_serializing_if = "is_default")]
        authentic: bool,
        /// IP address of the sender.
        ip_address: IpAddr,
        /// The chat message.
        message: String,
        /// Player id of sender.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        player_id: Option<PlayerId>,
        /// In Unix millis.
        /// Team name of the sender.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        team_name: Option<TeamName>,
        timestamp: NonZeroUnixMillis,
        /// Visitor ID of the sender or None if session/auth failed due to network issue.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visitor_id: Option<VisitorId>,
        /// Chat recipient.
        #[serde(default, skip_serializing_if = "is_default")]
        recipient: ChatRecipient,
    },
    /// Send a parley for consideration by other servers.
    SendServerMessage {
        /// The parley message.
        message: serde_json::Value,
        /// Server IDs of recipient servers (these must be of the same kind, local/cloud, as sender).
        recipients: HashSet<ServerId>,
    },
    /// A server has stopped. The server, its arenas, and their players are cleared.
    UnregisterServer,
    /// Update the leaderboards with recent scores, always in batches
    /// for efficiency and accuracy.  Cloud servers, local servers and
    /// realms all have separate leaderboards.
    UpdateLeaderboards {
        /// Realm ID.
        #[serde(default, skip_serializing_if = "is_default")]
        realm_id: RealmId,
        /// List of player scores to be used to update leaderboards.
        scores: Box<[LeaderboardScoreDto]>,
    },
    /// Update metrics, always in batches for efficiency. Sent every hour.
    UpdateMetrics {
        /// Timestamp of this batch of metrics.
        timestamp: NonZeroUnixMillis,
        /// Batch of metrics.
        #[serde(deserialize_with = "crate::serde_util::box_slice_skip_invalid")]
        metrics: Box<[(Option<MetricFilter>, EngineMetrics)]>,
    },
    UpdateQuestSamples {
        quest_samples: Box<[QuestSampleDto]>,
    },
    /// Update the server log, always in batches for efficiency.
    UpdateServerLog {
        /// A batch of trace log messages.
        server_log: Box<[ServerLogDto]>,
    },
}
