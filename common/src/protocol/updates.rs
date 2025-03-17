// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{
    InvitationRequest, InvitationUpdate, LeaderboardUpdate, LiveboardUpdate, SystemUpdate,
};
use crate::bitcode::{self, Decode, Encode};
use crate::{
    AdEvent, ArenaId, ArenaQuery, ChatMessage, ClaimValue, ClientActivity, Dedup, GameFence,
    MessageNumber, NonZeroUnixMillis, Owned, PlayerAlias, PlayerId, QuestEvent, ReconnectionToken,
    RegionId, ScopeClaimKey, ServerId, SessionToken, TeamId, TeamName, VisitorId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Chat related request from client to server.
#[derive(Clone, Debug, Encode, Decode)]
pub enum ChatRequest {
    /// Special meaning if moderator.
    Report(MessageNumber),
    /// Avoid seeing this player's messages.
    Mute(MessageNumber),
    /// Send a chat message.
    Send {
        message: String,
        /// Whether messages should only be visible to sender's team.
        whisper: bool,
    },
    /// Chat will be in safe mode for this many more minutes. For moderators only.
    SetSafeMode(u32),
    /// Chat will be in slow mode for this many more minutes. For moderators only.
    SetSlowMode(u32),
    /// Resume seeing this player's messages.
    Unmute(MessageNumber),
}

/// Chat related update from server to client.
#[derive(Clone, Debug, Encode, Decode)]
pub enum ChatUpdate {
    Muted(MessageNumber),
    PlayerRestricted { message_number: MessageNumber },
    Received(Box<[(MessageNumber, Dedup<MessageDto>)]>),
    SafeModeSet(u32),
    SlowModeSet(u32),
    Sent,
    Unmuted(MessageNumber),
    Reported(MessageNumber),
}

/// General request from client to server.
#[derive(Clone, Debug, Encode, Decode)]
pub enum ClientRequest {
    /// Present a Plasma session id.
    Login(SessionToken),
    /// An advertisement was shown or played.
    TallyAd(AdEvent),
    TallyFps(f32),
    /// This is distinct from lower level keepalive, which the web browser handles automatically.
    Heartbeat(ClientActivity),
    Quit,
    ArenaSettings(String),
    RecordQuestEvent(QuestEvent),
    /// Request a `Redirect` from the server. Used to preserve metrics/quests over arena switches.
    SwitchArena {
        server_id: ServerId,
        arena_id: ArenaQuery,
    },
    /// Configure join announcement.
    AnnouncementPreference(bool),
}

/// General update from server to client.
#[derive(Clone, Debug, Encode, Decode)]
pub enum ClientUpdate {
    /// Used in rare cases where the client loads too early.
    BootstrapSnippet(Owned<str>),
    LoggedIn(SessionToken),
    SessionCreated {
        server_id: ServerId,
        arena_id: ArenaId,
        region_id: Option<RegionId>,
        player_id: PlayerId,
        token: ReconnectionToken,
        date_created: NonZeroUnixMillis,
    },
    Redirect {
        server_id: ServerId,
        arena_id: ArenaId,
        player_id: PlayerId,
        token: ReconnectionToken,
    },
    /// Clear players, liveboard, leaderboard, servers, chat, game state, etc.
    /// (things subject to state sync).
    ClearSyncState {
        game_fence: GameFence,
    },
    /// A diff.
    UpdateClaims(HashMap<ScopeClaimKey, Option<ClaimValue>>),
}

/// Client to server request.
#[derive(Clone, Debug, Encode, Decode)]
pub enum CommonRequest<GR> {
    Chat(ChatRequest),
    Client(ClientRequest),
    Game(GR, Option<GameFence>),
    Invitation(InvitationRequest),
    /// Handled by the socket layer.
    Redial {
        query_string: Box<str>,
    },
}

#[cfg(feature = "server")]
impl<GR: Serialize + serde::de::DeserializeOwned + actix::Message> actix::Message
    for CommonRequest<GR>
where
    <GR as actix::Message>::Result: Serialize + serde::de::DeserializeOwned,
{
    type Result = CommonUpdate<GR::Result>;
}

/// Server to client update.
#[derive(Clone, Debug, Encode, Decode)]
#[cfg_attr(feature = "server", derive(actix::Message))]
#[cfg_attr(feature = "server", rtype(result = "()"))]
pub enum CommonUpdate<GU> {
    Chat(ChatUpdate),
    Client(ClientUpdate),
    Game(GU),
    Invitation(InvitationUpdate),
    Leaderboard(LeaderboardUpdate),
    Liveboard(LiveboardUpdate),
    Player(PlayerUpdate),
    System(SystemUpdate),
}

/// The Message Data Transfer Object (DTO) is used for chats.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct MessageDto {
    /// For display in case alias is changed or player quits.
    pub alias: PlayerAlias,
    /// For viewing profile.
    pub visitor_id: Option<VisitorId>,
    /// Don't use team_id in case team is deleted or ID re-used.
    pub team_name: Option<TeamName>,
    /// Nickname same as alias.
    pub authentic: bool,
    /// Authority (server).
    pub authority: bool,
    /// Whether message is directed to team/player only.
    pub whisper: bool,
    /// Content of the message.
    pub message: ChatMessage,
}

/// The Player Data Transfer Object (DTO) binds player ID to player data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct PlayerDto {
    pub alias: PlayerAlias,
    pub admin: bool,
    pub moderator: bool,
    pub player_id: PlayerId,
    pub team_id: Option<TeamId>,
    //pub visitor_id: Option<VisitorId>,
    //pub user_id: Option<UserId>,
    pub authentic: bool,
}

/// Player related update from server to client.
#[derive(Clone, Debug, Encode, Decode)]
pub enum PlayerUpdate {
    Reported(PlayerId),
    Updated {
        added: Owned<[PlayerDto]>,
        removed: Owned<[PlayerId]>,
    },
}
