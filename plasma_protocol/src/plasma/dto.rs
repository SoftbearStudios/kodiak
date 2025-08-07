// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    impl_wrapper_str, is_default, ArenaId, ClaimSubset, CohortId, DomainName, GameId, LanguageDto,
    LanguageId, NickName, NonZeroUnixMillis, PlayerId, Referrer, RegionId, ServerId, ServerNumber,
    ServerToken, UserAgentId, UserId, VisitorId,
};
use arrayvec::ArrayString;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug, Formatter};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChatRecipient {
    /// Broadcast to the arena (on a the server) that sent it.
    Arena,
    /// Broadcast to all arenas (on all servers) of a realm.
    #[default]
    Broadcast,
    Player(PlayerId),
    TeamOf(PlayerId),
    /// Only log the chat.
    None,
}

/// Sent in the `Heartbeat` for every player which has relevant claims, and in `Claims` update.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimUpdateDto {
    pub arena_id: ArenaId,
    pub claims: ClaimSubset,
    pub player_id: PlayerId,
    pub visitor_id: VisitorId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DomainDto {
    /// mazean.com is primary
    /// foo.com means mazean.foo.com is alternative
    pub domain: DomainName,
    pub certificate: Box<str>,
    pub private_key: Box<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileLoadedResult {
    Loaded {
        content_data: Vec<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
    },
    Forbidden,
    NotFound,
    TypeMismatch,
    Error(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileNamespace {
    NickName(NickName),
    RequestVisitorId,
}

/// Mirrors log::Level.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RealmAcl {
    UserBlacklist(HashSet<UserId>),
    VisitorBlacklist(HashSet<VisitorId>),
    VisitorWhitelist(HashSet<VisitorId>),
}

impl Default for RealmAcl {
    fn default() -> Self {
        Self::VisitorBlacklist(Default::default())
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerFailureDiagnostic(ArrayString<60>);
impl_wrapper_str!(ServerFailureDiagnostic);

impl ServerFailureDiagnostic {
    pub fn new(s: &str) -> Option<Self> {
        ArrayString::from_str(s).ok().map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerLogDto {
    /// Milliseconds.
    pub timestamp: NonZeroUnixMillis,
    /// The log level/severity.
    pub level: LogLevel,
    /// The code generating the log entry.
    pub source: String,
    /// The log entry.
    pub message: String,
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ServerRole {
    /// Server is being deleted from virtual hosting and database.
    Deleting,
    /// Server automatically closed to new players but existing players may continue playing.
    /// - Redirects all new connections to specified (presumably active) server (admins should
    ///   never need to specify *which* active server to redirect to).
    /// - Hidden from server selector.
    /// - Possibly unhealthy, it will stay in this state for at least 24h to allow diagnostics.
    /// - After sufficient time has passed, if the number of players ever falls to zero,
    ///   it will change to 'Deleting'.
    Failed {
        date_failed: NonZeroUnixMillis,
        #[serde(skip_serializing_if = "Option::is_none")]
        diagnostic: Option<ServerFailureDiagnostic>,
        #[serde(skip_serializing_if = "Option::is_none")]
        redirect: Option<ServerNumber>,
    },
    /// - New connections are routed via DNS once booting is complete.
    /// - Accepts new connections from public website
    /// - Optionally advertises a more optimal active server, depending on the player
    /// - Displayed in server selector when client hash is compatible
    /// - Never redirects to other servers
    /// - Is (was recently) healthy
    Public,
    /// - New connections are routed via DNS once booting is complete.
    /// - Accepts new connections from realm websites
    /// - Never redirects to other servers
    /// - Is (was recently) healthy
    /// - If the number of realms falls to zero, it may be de-allocated.
    Realms,
    /// Server manually closed to new players but remains available to be
    /// conscripted as a new public server and any existing players may
    /// continue to play in the meantime.
    /// - Redirects all new connections to specified (presumably active) server (admins should
    ///   never need to specify *which* active server to redirect to).
    /// - Hidden from server selector.
    Standby {
        #[serde(skip_serializing_if = "Option::is_none")]
        redirect: Option<ServerNumber>,
    },
    /// Server manually closed to new players but existing players may continue playing.
    /// - Redirects all new connections to specified (presumably active) server (admins should
    ///   never need to specify *which* active server to redirect to).
    /// - Hidden from server selector.
    /// - If the number of players ever falls to zero, it will either change to 'Deleting'
    ///   or else it will be re-allocated as 'Public', 'Realms', or 'Unlisted'.
    Terminating {
        #[serde(skip_serializing_if = "Option::is_none")]
        redirect: Option<ServerNumber>,
    },
    /// - Accepts new connections
    /// - Hidden from server selector
    /// - Never redirects to other servers
    /// - Possibly unhealthy; servers reset to this state if they don't hear from plasma
    #[default]
    Unlisted,
}

impl ServerRole {
    /// Closing (failed, standy, terminating) with or without redirection.
    pub fn is_closing(self) -> bool {
        match self {
            Self::Failed { .. } | Self::Standby { .. } | Self::Terminating { .. } => true,
            _ => false,
        }
    }

    pub fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }

    pub fn is_realms(self) -> bool {
        matches!(self, Self::Realms)
    }

    pub fn is_redirected(self) -> bool {
        Self::redirect(self).is_some()
    }

    pub fn is_unlisted(self) -> bool {
        matches!(self, Self::Unlisted)
    }

    pub fn redirect(self) -> Option<ServerNumber> {
        match self {
            Self::Failed { redirect, .. } => redirect,
            Self::Standby { redirect, .. } => redirect,
            Self::Terminating { redirect, .. } => redirect,
            _ => None,
        }
    }
}

impl Debug for ServerRole {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ServerRole::Deleting => f.write_str("deleting"),
            ServerRole::Failed {
                redirect: Some(redirect),
                ..
            } => {
                write!(f, "failed (redirect to {})", redirect.0)
            }
            ServerRole::Failed { redirect: None, .. } => f.write_str("failed"),
            ServerRole::Public => f.write_str("public"),
            ServerRole::Realms => f.write_str("realms"),
            ServerRole::Standby {
                redirect: Some(redirect),
            } => write!(f, "standby (redirect to {})", redirect.0),
            ServerRole::Standby { redirect: None } => write!(f, "standby"),
            ServerRole::Terminating {
                redirect: Some(redirect),
            } => write!(f, "terminating (redirect to {})", redirect.0),
            ServerRole::Terminating { redirect: None } => write!(f, "terminating"),
            ServerRole::Unlisted => f.write_str("unlisted"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub content: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub criteria: SnippetCriteria,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetCriteria {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohort_id: Option<CohortId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_id: Option<GameId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<Referrer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_id: Option<RegionId>,
    // For example (1500, 1700)
    //#[serde(default, skip_serializing_if = "Option::is_none")]
    //pub time_range: Option<(usize, usize)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent_id: Option<UserAgentId>,
    /// If matched, keep going.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fallthrough: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TranslationsDto {
    /// If `bulktext` is true, then English text is not hard-coded and
    /// `translation_id` should be `Some`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub bulktext: bool,
    /// If `translation_id` is `Some`, it should be used as the ID.  (This may
    /// be done to share the translation with the softbear.com website, or to
    /// disambiguate equivalent English text, or when `bulktext` is true.)  If
    /// `translation_id` is `None`, use the hard-coded English text as the ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_id: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub translated_text: HashMap<LanguageId, String>,
}

/// A URL to a file with this format is sent in the `Translations` update.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TranslationsFile {
    // Languages are arranged in whatever order menus should use.
    #[serde(default, skip_serializing_if = "is_default")]
    pub languages: Box<[LanguageDto]>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub translations: Box<[TranslationsDto]>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsocketConnectQuery {
    pub game_id: GameId,
    pub server_id: ServerId,
    pub server_token: ServerToken,
}
