// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::RealmAcl;
use crate::{is_default, ArenaId, RealmId, RegionId, SceneId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A collection of arenas that share chat and, ideally, liveboard.
pub struct RealmUseTopology {
    #[serde(default, skip_serializing_if = "is_default")]
    pub acl: RealmAcl,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub scenes: HashMap<SceneId, SceneUseTopology>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A scene (tier, instance) of a realm.  If deleted, an arena *may* still exist
/// on the server (until garbage collected) but is not open to new/redirected players.
pub struct SceneUseTopology {
    /// Last server-reported player count, obtained from `arenas` in `Heartbeat`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub player_count: u16,
    /// JSON object containing settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerUseTopology {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub datacenter: String,
    /// The default, public realm i.e. realm_id: None, from game table `topology` field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_realm: Option<RealmUseTopology>,
    /// From game table `topology` field.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub other_realms: HashMap<RealmId, RealmUseTopology>,
    /// Continental region where server is located. For example, "North America".
    pub region_id: RegionId,
    // Territory served by server (often smaller than region). For example, "Western USA".
    //pub territory_id: TerritoryId,
}

impl ServerUseTopology {
    pub fn realm(&self, realm_id: RealmId) -> Option<&RealmUseTopology> {
        if realm_id.is_public_default() {
            self.default_realm.as_ref()
        } else {
            self.other_realms.get(&realm_id)
        }
    }

    pub fn realms(&self) -> impl Iterator<Item = (RealmId, &RealmUseTopology)> {
        self.default_realm
            .as_ref()
            .map(|r| (RealmId::PublicDefault, r))
            .into_iter()
            .chain(self.other_realms.iter().map(|(k, v)| (*k, v)))
    }

    pub fn realms_mut(&mut self) -> impl Iterator<Item = (RealmId, &mut RealmUseTopology)> {
        self.default_realm
            .as_mut()
            .map(|r| (RealmId::PublicDefault, r))
            .into_iter()
            .chain(self.other_realms.iter_mut().map(|(k, v)| (*k, v)))
    }

    pub fn arena(&self, arena_id: ArenaId) -> Option<&SceneUseTopology> {
        self.realm(arena_id.realm_id)
            .and_then(|realm| realm.scenes.get(&arena_id.scene_id))
    }

    pub fn arenas(&self) -> impl Iterator<Item = (ArenaId, &SceneUseTopology)> {
        self.realms().flat_map(|(realm_id, realm)| {
            realm
                .scenes
                .iter()
                .map(move |(scene_id, scene)| (ArenaId::new(realm_id, *scene_id), scene))
        })
    }
}
