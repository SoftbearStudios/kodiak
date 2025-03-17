// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{ChatRecipient, ClaimUpdateDto, DomainDto, ServerRole, ServerUseTopology, Snippet};
use crate::{
    is_default, ArenaId, ArenaToken, ChatId, ChatMessage, LeaderboardScoreDto, NickName, PeriodId,
    PlayerAlias, PlayerId, RealmId, Referrer, ServerId, SessionToken, TeamName, TeamToken,
    VisitorId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::IpAddr;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(actix::Message))]
#[cfg_attr(feature = "server", rtype(result = "()"))]
pub enum PlasmaUpdate {
    /// Version 1 protocol.
    V1(
        #[serde(deserialize_with = "crate::serde_util::box_slice_skip_invalid")]
        Box<[PlasmaUpdateV1]>,
    ),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaUpdateV1 {
    /// Sent after [`RegisterArena`] where [`realm_id`] is [`Some`] and when updated.
    Arena {
        /// Navigational ID which includes realm, tier, etc.
        #[serde(default)]
        arena_id: ArenaId,
        /// Uniquely identifies the arena, for idempotency.
        arena_token: ArenaToken,
    },
    /// Sent after [`SendChat`] on on same or another server,
    /// providing the profanity filter passes.
    Chat {
        /// Message originated from admin or profanity filter.
        #[serde(default, skip_serializing_if = "is_default")]
        admin: bool,
        /// Alias of the sender.
        alias: PlayerAlias,
        #[serde(default, skip_serializing_if = "is_default")]
        authentic: bool,
        /// The unique ID of this chat message. Contains `ServerId`, `ArenaId`, and timestamp.
        chat_id: ChatId,
        /// This allows users to block each other.
        ip_address: IpAddr,
        /// The chat message.
        message: ChatMessage,
        /// Player who sent the message or `None` for admin message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        player_id: Option<PlayerId>,
        /// Recipient filter.
        #[serde(default, skip_serializing_if = "is_default")]
        recipient: ChatRecipient,
        /// Team name of the sender or `None` if sender is not in a team.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        team_name: Option<TeamName>,
        /// Visitor ID of the sender or `None` if it is unknown or not applicable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visitor_id: Option<VisitorId>,
    },
    /// Sent for non-signed in players after [`AuthenticatePlayer`].
    /// May also be sent for any player after [`Heartbeat`].
    Claims {
        #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
        claims: Box<[ClaimUpdateDto]>,
    },
    Domains {
        domains: Box<[DomainDto]>,
    },
    /// Acknowledges a received heartbeat so the server knows it got through.
    //
    // {} is for backward compatibility
    Heartbeat {},
    /// The leaderboard for the specified period.  This is sent
    /// in response to [`RegisterServer`], and also when a leaderboard
    /// has changed due to [`UpdateLeaderboards`] from any game server.
    Leaderboard {
        /// Period to which the leaderboard applies (e.g. daily).
        period_id: PeriodId,
        /// Realm ID of the leaderboard.
        #[serde(default, skip_serializing_if = "is_default")]
        realm_id: RealmId,
        /// The scores that made the leaderboard.
        scores: Box<[LeaderboardScoreDto]>,
    },
    /// Sent after [`SendServerMessage`] from another server which is of the
    /// same kind (local/cloud).
    Parley {
        /// The message, expressed in JSON which is somewhat flexible to protocol changes.
        message: serde_json::Value,
        /// Server number of sender.
        sender: ServerId,
    },
    /// Sent for players after [`AuthenticatePlayer`].  For backward compatibility, only signed in players.
    Player {
        /// Player should be included in active heartbeat e.g. because they are a user.
        #[serde(default, skip_serializing_if = "is_default")]
        active_heartbeat: bool,
        /// True if player has in-game admin priviledges.
        #[serde(default, skip_serializing_if = "is_default")]
        admin: bool,
        /// ArenaId of player.
        #[serde(default, skip_serializing_if = "is_default")]
        arena_id: ArenaId,
        /// Uniquely identifies the arena, for idempotency.
        arena_token: ArenaToken,
        /// True if user is NOT allowed to play in arena/realm.
        #[serde(default, skip_serializing_if = "is_default")]
        ban: bool,
        /// True if player has in-game moderator priviledges.
        #[serde(default, skip_serializing_if = "is_default")]
        moderator: bool,
        /// Unique nick name of signed in user, if any, otherwise None.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nick_name: Option<NickName>,
        /// Id of player, for efficient lookup.
        player_id: PlayerId,
        /// The `session_token` that was used by `AuthenticatePlayer`.
        session_token: SessionToken,
        /// True if player is signed in.
        #[serde(default, skip_serializing_if = "is_default")]
        user: bool,
        /// Visitor ID of player.
        visitor_id: VisitorId,
    },
    Quests {
        /// Fraction to sample.
        /// 0 = none
        /// 0.1 = 10%
        /// 1 = all
        #[serde(default, skip_serializing_if = "is_default")]
        fraction: f32,
    },
    /// Sent after each [`Heartbeat`] (to be self-healing), and when updated.
    Role {
        /// Used to be an option in a larger message, but that is no longer useful.
        /// Just stop wrapping in `Some`.
        role: ServerRole,
    },
    Snippets {
        snippets: Box<[Snippet]>,
    },
    TeamName {
        /// Arena ID of requestor.
        #[serde(default, skip_serializing_if = "is_default")]
        arena_id: ArenaId,
        /// Player ID of requestor.
        player_id: PlayerId,
        /// The team name that was reserved.
        team_name: TeamName,
        /// Proof that the team name was reserved.
        team_token: TeamToken,
    },
    /// List of all game servers relevant to the recipient game server.
    /// Relevant means the recipient hosts something that players of
    /// recipient could be redirected or travel to. This implies matching
    /// client hashes.
    ///
    /// Sent in response to [`RegisterServer`] and [`Heartbeat`] (self-healing)
    /// OR when updated by the heartbeat of another server (e.g. player
    /// count change), the latter of which is more frequent when there
    /// are more servers.
    ///
    /// All servers in the list satisfy the following conditions:
    /// - Cloud (except, to local servers, include local too)
    /// - [`RegionId`] is known
    /// - Client hash is compatible
    /// - Public role (consider all local servers public)
    /// - Healthy
    /// - Recent enough heartbeat
    ///
    /// Exception: Server receives this message, it should be included regardless
    /// of role, health, or heartbeat. Client hash is trivially compatible. All
    /// other conditions still apply.
    ///
    Topology {
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        servers: HashMap<ServerId, ServerUseTopology>,
    },
    /// Which items to track in metrics.
    Track {
        /// Map of alias to preferred referrer.
        no_referrer: Option<Referrer>,
        other_referrer: Option<Referrer>,
        referrers: HashMap<Referrer, Referrer>,
    },
    /// Translations of phrases and bulk text.
    Translations {
        /// The URL of a JSON serialized `TranslationsFile`.
        file_url: String,
    },
    // Diagnostic message sent from Plasma to game server.
    Warning {
        message: String,
    },
}
