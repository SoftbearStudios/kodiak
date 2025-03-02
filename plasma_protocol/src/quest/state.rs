// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::QuestEventDto;
use crate::{
    is_default, ArenaId, CohortId, LanguageId, LifecycleId, NavigationMetricsDto, PlayerAlias,
    Referrer, RegionId, ServerId, UserAgentId,
};
use bitcode::{Decode, Encode};
use cub::NonZeroUnixMillis;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestSampleDto {
    /// When the quest started.
    pub date_created: NonZeroUnixMillis,
    /// First time a visitor loaded the game.
    ///
    pub date_visitor_created: NonZeroUnixMillis,
    #[serde(default, skip_serializing_if = "is_default")]
    pub cohort_id: CohortId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<Referrer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_id: Option<RegionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent_id: Option<UserAgentId>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub language_id: LanguageId,
    #[serde(default, skip_serializing_if = "is_default")]
    pub lifecycle_id: Option<LifecycleId>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub navigation: NavigationMetricsDto,
    /// Server where the quest began.
    pub server_id: ServerId,
    /// Arena where the quest began.
    #[serde(default)]
    pub arena_id: ArenaId,
    #[serde(
        default,
        skip_serializing_if = "<[_]>::is_empty",
        deserialize_with = "crate::serde_util::box_slice_skip_invalid"
    )]
    pub events: Box<[QuestEventDto]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum QuestState {
    Spawning {},
    Playing {
        alias: PlayerAlias,
        /// Initial score.
        score: u32,
    },
    Dead {
        reason: Box<str>,
    },
}

impl Default for QuestState {
    fn default() -> Self {
        Self::Spawning {}
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum FatalError {
    WebGl,
    WebGl2,
}

/// Must match `renderer` errors.
impl From<String> for FatalError {
    fn from(s: String) -> Self {
        if s.contains("WebGL2") {
            Self::WebGl2
        } else {
            Self::WebGl
        }
    }
}
