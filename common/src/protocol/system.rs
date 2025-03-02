// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    actix_response, is_default, ArenaQuery, CohortId, DomainName, LanguageDto, LanguageId,
    NonZeroUnixMillis, Owned, Referrer, ServerId, SessionToken,
};
use arrayvec::ArrayString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ArenaSettingsDto<GAS: Default + PartialEq> {
    #[serde(flatten)]
    pub engine: EngineArenaSettings,
    #[serde(flatten)]
    pub game: GAS,
}

impl<GAS: Default + PartialEq> Deref for ArenaSettingsDto<GAS> {
    type Target = EngineArenaSettings;

    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

impl<GAS: Default + PartialEq> DerefMut for ArenaSettingsDto<GAS> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.engine
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineArenaSettings {
    /// How many bots; `None` means "auto".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bots: Option<u16>,
    /// Bot aggression, from 0.0 to 1 (default) to 10.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_aggression: Option<f32>,
}

impl EngineArenaSettings {
    pub fn bot_aggression(&self) -> f32 {
        self.bot_aggression.unwrap_or(1.0)
    }
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoGameArenaSettings {}

/// Initiate a WebSocket/WebTransport with these optional parameters in the URL query string.
///
/// Warning `#[serde(flatten)]` is not supported (https://github.com/nox/serde_urlencoded/issues/33).
#[derive(Debug, Serialize, Deserialize)]
pub struct SocketQuery {
    #[serde(default, skip_serializing_if = "is_default")]
    pub arena_id: ArenaQuery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<SessionToken>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<Referrer>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub cohort_id: CohortId,
    #[serde(default, skip_serializing_if = "is_default")]
    pub language_id: LanguageId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_created: Option<NonZeroUnixMillis>,
    /// Hours relative to UTC, in minutes.
    #[serde(default, skip_serializing_if = "is_default")]
    pub timezone_offset: i16,
    /// WebTransport cannot be trusted to pass this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<ArrayString<188>>,
    /// DNS query latency.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dns: u16,
    /// TCP establishment latency, not counting TLS (if any).
    #[serde(default, skip_serializing_if = "is_default")]
    pub tcp: u16,
    /// TLS establishment latency.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tls: u16,
    /// HTTP request and response latency total.
    #[serde(default, skip_serializing_if = "is_default")]
    pub http: u16,
    /// DOM loading latency, after HTTP response.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dom: u16,
}

/// Pass the following query parameters to the system endpoint to inform server routing.
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemQuery {
    /// Express a [`ServerId`] preference. `None` means unknown.
    /// It is not guaranteed to be honored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_id: Option<ServerId>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub arena_id: ArenaQuery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<Referrer>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub cohort_id: CohortId,
    /// Get a head start on translations.
    #[serde(flatten)]
    pub translation: TranslationRequest,
}

/// Response to system request.
#[derive(Serialize, Deserialize)]
pub struct SystemResponse {
    /// The [`ServerId`] matching the invitation, or closest to the client.
    pub server_id: ServerId,
    pub languages: Owned<[LanguageDto]>,
    pub snippets: Box<[Owned<str>]>,
    /// A list of servers that might accept an invited player.
    pub available_servers: Owned<[ServerId]>,
    pub alternative_domains: Owned<[DomainName]>,
    #[serde(flatten)]
    pub translation: TranslationResponse,
}

actix_response!(SystemResponse);

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(actix::Message))]
#[cfg_attr(feature = "server", rtype(result = "TranslationResponse"))]
pub struct TranslationRequest {
    #[serde(default, skip_serializing_if = "is_default")]
    pub language_id: LanguageId,
}

/// Response to translation request.
#[derive(Serialize, Deserialize)]
pub struct TranslationResponse {
    pub translations: Owned<HashMap<String, String>>,
}

actix_response!(TranslationResponse);
