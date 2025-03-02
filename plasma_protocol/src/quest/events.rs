// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{FatalError, QuestState};
use crate::{is_default, ArenaQuery, NexusPath, ServerId};
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum AdEvent {
    Banner(BannerAdEvent),
    Interstitial(VideoAdEvent),
    Rewarded(VideoAdEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum BannerAdEvent {
    Request,
    Show,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub enum ClientActivity {
    /// Tab is invisible.
    Hidden,
    /// Tab is visible but no inputs have been made recently.
    Afk,
    /// Player is actively making inputs.
    #[default]
    Active,
}

impl ClientActivity {
    pub fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    pub fn is_afk(&self) -> bool {
        matches!(self, Self::Afk)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum QuestEvent {
    Ad {
        ad: AdEvent,
    },
    /// Afk, etc.
    Activity {
        activity: ClientActivity,
    },
    Arena {
        #[serde(rename = "serverId")]
        server_id: ServerId,
        #[serde(rename = "arenaId")]
        arena_id: ArenaQuery,
        /// Initiated by game.
        #[serde(default, skip_serializing_if = "is_default")]
        game: bool,
    },
    /// Arena closing.
    Closing {
        #[serde(default, skip_serializing_if = "is_default")]
        closing: bool,
    },
    Chat {
        #[serde(default, skip_serializing_if = "is_default")]
        whisper: bool,
    },
    Error {
        error: FatalError,
    },
    Trace {
        #[serde(default, skip_serializing_if = "str::is_empty")]
        message: Box<str>,
    },
    Fps {
        fps: f32,
    },
    Nexus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<NexusPath>,
    },
    Rtt {
        rtt: u16,
    },
    /// According to the game.
    #[serde(alias = "Kill")]
    Victory {
        /// Killed a bot (not a human player).
        #[serde(default, skip_serializing_if = "is_default")]
        bot: bool,
        /// Score of killed player.
        #[serde(default, skip_serializing_if = "is_default")]
        score: u32,
    },
    /// Don't send every point earned, maybe every power of 10.
    Score {
        score: u32,
    },
    Socket {
        #[serde(default, skip_serializing_if = "is_default")]
        open: bool,
        #[serde(
            default,
            rename = "supportsUnreliable",
            skip_serializing_if = "is_default"
        )]
        supports_unreliable: bool,
    },
    State {
        state: QuestState,
    },
    Team {
        #[serde(default, skip_serializing_if = "is_default")]
        joined: bool,
    },
    /// Tutorial progress.
    Tutorial {
        /// For games with two instructions, 1 and 2 are sent.
        #[serde(default, skip_serializing_if = "is_default")]
        step: u8,
    },
    Upgrade {
        level: u32,
    },
}

impl QuestEvent {
    pub const TRACE_LIMIT: usize = 1024;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "camelCase")]
pub struct QuestEventDto {
    pub t: u64,
    pub e: QuestEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum VideoAdEvent {
    Request,
    /// Currently unused.
    Start,
    Finish,
    Cancel,
}
