// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{is_default, PlayerId, SceneId, VisitorId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

/// Sent in `ArenaHeartbeat`for each signed in player in the arena.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveHeartbeat {
    /// TODO: this is an Option for backward compatibility.
    pub visitor_id: Option<VisitorId>,
}

/// Sent in `RealmHeartbeat` for each arena (scene) on the server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArenaHeartbeat {
    /// The user ID of each player that has signed in.
    #[serde(default, skip_serializing_if = "is_default")]
    pub actives: HashMap<PlayerId, ActiveHeartbeat>,
    /// Player count will exceed the length of `actives` when some players did not sign in.
    #[serde(default, skip_serializing_if = "is_default")]
    pub player_count: u16,
    /// JSON object containing new settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    /// Seconds per tick.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tick_duration: f32,
}

/// The heartbeats for every scene in the realm.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RealmHeartbeat {
    #[serde(default, skip_serializing_if = "is_default")]
    pub scenes: BTreeMap<SceneId, ArenaHeartbeat>,
}
