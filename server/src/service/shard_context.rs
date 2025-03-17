// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{Arena, LiveboardRepo, SceneRepo};
use crate::{ArenaService, Player, PlayerId, SceneId};

/// A shard has its own ~~chat and~~ liveboard. Depending on the game,
/// a single server may support one shard per realm or one shard per scene.
pub struct ShardContext<G: ArenaService> {
    pub(crate) liveboard: LiveboardRepo<G>,
}

impl<G: ArenaService> Default for ShardContext<G> {
    fn default() -> Self {
        Self {
            liveboard: Default::default(),
        }
    }
}

#[allow(unused)]
pub trait ShardContextProvider<G: ArenaService<Shard = Self>> {
    const PER_SCENE: bool = std::mem::size_of::<Self::PerScene>() > 0;
    type PerRealm: Default + Unpin;
    type PerScene: Default + Unpin;

    fn shard_context<'a>(
        per_realm: &'a Self::PerRealm,
        per_scene: &'a Self::PerScene,
    ) -> &'a ShardContext<G>;
    fn shard_context_mut<'a>(
        per_realm: &'a mut Self::PerRealm,
        per_scene: &'a mut Self::PerScene,
    ) -> &'a mut ShardContext<G>;
    fn realm_shard_context(per_realm: &Self::PerRealm) -> Option<&ShardContext<G>>;
    fn realm_shard_context_mut(per_realm: &mut Self::PerRealm) -> Option<&mut ShardContext<G>>;
    fn scene_shard_context(per_scene: &Self::PerScene) -> Option<&ShardContext<G>>;
    fn scene_shard_context_mut(per_scene: &mut Self::PerScene) -> Option<&mut ShardContext<G>>;
}

pub struct ShardPerRealm;

impl<G: ArenaService<Shard = Self>> ShardContextProvider<G> for ShardPerRealm {
    type PerRealm = ShardContext<G>;
    type PerScene = ();

    fn shard_context<'a>(
        per_realm: &'a Self::PerRealm,
        _per_scene: &'a Self::PerScene,
    ) -> &'a ShardContext<G> {
        per_realm
    }

    fn shard_context_mut<'a>(
        per_realm: &'a mut Self::PerRealm,
        _per_scene: &'a mut Self::PerScene,
    ) -> &'a mut ShardContext<G> {
        per_realm
    }

    fn realm_shard_context(per_realm: &Self::PerRealm) -> Option<&ShardContext<G>> {
        Some(per_realm)
    }

    fn realm_shard_context_mut(per_realm: &mut Self::PerRealm) -> Option<&mut ShardContext<G>> {
        Some(per_realm)
    }

    fn scene_shard_context(_per_scene: &Self::PerScene) -> Option<&ShardContext<G>> {
        None
    }

    fn scene_shard_context_mut(_per_scene: &mut Self::PerScene) -> Option<&mut ShardContext<G>> {
        None
    }
}

pub struct ShardPerTier;

impl<G: ArenaService<Shard = Self>> ShardContextProvider<G> for ShardPerTier {
    type PerRealm = ();
    type PerScene = ShardContext<G>;

    fn shard_context<'a>(
        _per_realm: &'a Self::PerRealm,
        per_scene: &'a Self::PerScene,
    ) -> &'a ShardContext<G> {
        per_scene
    }

    fn shard_context_mut<'a>(
        _per_realm: &'a mut Self::PerRealm,
        per_scene: &'a mut Self::PerScene,
    ) -> &'a mut ShardContext<G> {
        per_scene
    }

    fn realm_shard_context(_per_realm: &Self::PerRealm) -> Option<&ShardContext<G>> {
        None
    }

    fn realm_shard_context_mut(_per_realm: &mut Self::PerRealm) -> Option<&mut ShardContext<G>> {
        None
    }

    fn scene_shard_context(per_scene: &Self::PerScene) -> Option<&ShardContext<G>> {
        Some(per_scene)
    }

    fn scene_shard_context_mut(per_scene: &mut Self::PerScene) -> Option<&mut ShardContext<G>> {
        Some(per_scene)
    }
}

pub type Query = (SceneId, PlayerId);
pub trait LiveboardCohort<G: ArenaService> {
    fn get_mut(&mut self, query: Query) -> Option<&mut Player<G>>;
    fn visit_mut(&mut self, visitor: impl FnMut(Query, &mut Player<G>));
}
impl<G: ArenaService> LiveboardCohort<G> for SceneRepo<G> {
    fn get_mut(&mut self, query: Query) -> Option<&mut Player<G>> {
        self.scenes
            .get_mut(&query.0)
            .and_then(|scene| scene.arena.arena_context.players.get_mut(query.1))
    }

    fn visit_mut(&mut self, mut visitor: impl FnMut(Query, &mut Player<G>)) {
        for (scene_id, scene) in self.iter_mut() {
            for (player_id, player) in scene.arena.arena_context.players.iter_mut() {
                visitor((scene_id, player_id), player);
            }
        }
    }
}

impl<G: ArenaService> LiveboardCohort<G> for Arena<G> {
    fn get_mut(&mut self, query: Query) -> Option<&mut Player<G>> {
        self.arena_context.players.get_mut(query.1)
    }

    fn visit_mut(&mut self, mut visitor: impl FnMut(Query, &mut Player<G>)) {
        for (player_id, player) in self.arena_context.players.iter_mut() {
            // TODO: impl on `(SceneId, Arena<G>)` instead of using topology.
            visitor(
                (
                    self.arena_context.topology.local_arena_id.scene_id,
                    player_id,
                ),
                player,
            );
        }
    }
}
