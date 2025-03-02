// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::arena_context::SendPlasmaRequest;
use super::scene_repo::Scene;
use super::shard_context::ShardContextProvider;
use super::{ChatRepo, InvitationRepo};
use crate::actor::{ClientStatus, PlasmaActlet};
use crate::observer::ObserverUpdate;
use crate::rate_limiter::{RateLimiterProps, RateLimiterState};
use crate::service::{Arena, ArenaService, LeaderboardRepo, SceneRepo};
use crate::{ArenaId, RealmId, ServerId};
use log::info;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// TODO: was pub(crate)
pub struct RealmRepo<G: ArenaService> {
    bots: Option<u16>,
    realms: HashMap<RealmId, Realm<G>>,
    collect_rate_limit: RateLimiterState,
}

// TODO: was pub(crate)
pub struct Realm<G: ArenaService> {
    pub(crate) scene_repo: SceneRepo<G>,
    pub(crate) realm_context: RealmContext<G>,
}

impl<G: ArenaService> Default for Realm<G> {
    fn default() -> Self {
        Self {
            scene_repo: Default::default(),
            realm_context: Default::default(),
        }
    }
}

pub struct RealmContext<G: ArenaService> {
    pub(crate) per_realm: <G::Shard as ShardContextProvider<G>>::PerRealm,
    pub(crate) chat: ChatRepo<G>,
    pub(crate) leaderboard: LeaderboardRepo<G>,
}

impl<G: ArenaService> Default for RealmContext<G> {
    fn default() -> Self {
        Self {
            leaderboard: Default::default(),
            chat: Default::default(),
            per_realm: Default::default(),
        }
    }
}

impl<G: ArenaService> RealmRepo<G> {
    pub(crate) fn new(bots: Option<u16>) -> Self {
        Self {
            bots,
            realms: HashMap::new(),
            collect_rate_limit: Default::default(),
        }
    }
}

impl<G: ArenaService> RealmRepo<G> {
    #[allow(unused)]
    pub(crate) fn main(&self) -> Option<&Scene<G>> {
        self.get(Default::default())
    }

    pub(crate) fn main_mut(&mut self) -> Option<&mut Scene<G>> {
        self.get_mut(Default::default())
    }

    #[allow(unused)]
    pub(crate) fn contains(&self, arena_id: ArenaId) -> bool {
        self.get(arena_id).is_some()
    }

    #[allow(unused)]
    pub(crate) fn realm(&self, realm_id: RealmId) -> Option<&Realm<G>> {
        self.realms.get(&realm_id)
    }

    pub(crate) fn realm_mut(&mut self, realm_id: RealmId) -> Option<&mut Realm<G>> {
        self.realms.get_mut(&realm_id)
    }

    pub(crate) fn get(&self, arena_id: ArenaId) -> Option<&Scene<G>> {
        self.realm(arena_id.realm_id)
            .and_then(|r| r.scene_repo.scenes.get(&arena_id.scene_id))
    }

    pub(crate) fn get_mut(&mut self, arena_id: ArenaId) -> Option<&mut Scene<G>> {
        self.realm_mut(arena_id.realm_id)
            .and_then(|r| r.scene_repo.scenes.get_mut(&arena_id.scene_id))
    }

    pub(crate) fn get_mut_with_context(
        &mut self,
        arena_id: ArenaId,
    ) -> Option<(&mut Scene<G>, &mut RealmContext<G>)> {
        self.realm_mut(arena_id.realm_id).and_then(
            |Realm {
                 scene_repo: realm,
                 realm_context: context,
             }| {
                realm
                    .scenes
                    .get_mut(&arena_id.scene_id)
                    .map(|a| (a, context))
            },
        )
    }

    #[allow(unused)]
    pub(crate) fn main_mut_with_context(
        &mut self,
    ) -> Option<(&mut Scene<G>, &mut RealmContext<G>)> {
        self.get_mut_with_context(ArenaId::default())
    }

    pub(crate) fn get_mut_or_default(
        &mut self,
        server_id: ServerId,
        arena_id: ArenaId,
        send_plasma_request: SendPlasmaRequest,
    ) -> (&mut RealmContext<G>, &mut Scene<G>) {
        let context_realm = self.realms.entry(arena_id.realm_id).or_default();
        let scene = context_realm
            .scene_repo
            .scenes
            .entry(arena_id.scene_id)
            .or_insert_with(|| {
                let mut arena = Arena::new(server_id, arena_id, send_plasma_request);
                arena.arena_context.settings.bots = self.bots;
                Scene {
                    arena,
                    per_scene: Default::default(),
                }
            });
        (&mut context_realm.realm_context, scene)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (ArenaId, &Scene<G>)> {
        self.realms.iter().flat_map(|(&realm_id, t)| {
            t.scene_repo.scenes.iter().map(move |(scene_id, v)| {
                (
                    ArenaId {
                        realm_id,
                        scene_id: *scene_id,
                    },
                    v,
                )
            })
        })
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (ArenaId, &mut Scene<G>)> {
        self.realms.iter_mut().flat_map(|(&realm_id, t)| {
            t.scene_repo.scenes.iter_mut().map(move |(scene_id, v)| {
                (
                    ArenaId {
                        realm_id,
                        scene_id: *scene_id,
                    },
                    v,
                )
            })
        })
    }

    #[allow(unused)]
    pub(crate) fn realms(&self) -> impl Iterator<Item = (RealmId, &Realm<G>)> {
        self.realms.iter().map(|(k, v)| (*k, v))
    }

    pub(crate) fn realms_mut(&mut self) -> impl Iterator<Item = (RealmId, &mut Realm<G>)> {
        self.realms.iter_mut().map(|(k, v)| (*k, v))
    }

    /// Internally rate-limited for performance.
    pub(crate) fn collect_arenas(
        &mut self,
        server_id: ServerId,
        invitations: &mut InvitationRepo<G>,
        plasma: &PlasmaActlet,
    ) {
        let now = Instant::now();
        if self
            .collect_rate_limit
            .should_limit_rate_with_now(&RateLimiterProps::new_pure(Duration::from_secs(1)), now)
        {
            return;
        }
        self.realms.retain(|&realm_id, context_realm| {
            context_realm.scene_repo.scenes.retain(|&scene_id, scene| {
                let arena_id = ArenaId { realm_id, scene_id };
                let sanctioned = plasma.is_sanctioned(server_id, arena_id);
                let was_sanctioned =
                    (now - scene.arena.arena_context.last_sanctioned) < Duration::from_secs(120);
                let active = scene.arena.arena_context.players.values().any(|player| {
                    player
                        .client()
                        .map(|_| {
                            player
                                .not_alive_duration()
                                // For temporary realms, which can be recreated if the browser refreshes, it
                                // doesn't make sense to close the arena when players are dead.
                                .map(|d| {
                                    arena_id.realm_id.is_temporary() || d < Duration::from_secs(60)
                                })
                                .unwrap_or(true)
                        })
                        .unwrap_or(false)
                });
                if sanctioned || was_sanctioned || active {
                    if sanctioned {
                        scene.arena.arena_context.last_sanctioned = now;
                    }
                    true
                } else {
                    for (_, player) in scene.arena.arena_context.players.iter() {
                        if let Some(client) = player.client()
                            && let ClientStatus::Connected { observer, .. } = &client.status
                        {
                            // This is likely redundant with dropping the channel.
                            let _ = observer.send(ObserverUpdate::Close);
                        }
                    }
                    info!("{server_id:?} stopping realm {realm_id:?} ({sanctioned}, {active})");
                    invitations.forget_arena_invitations(arena_id);
                    false
                }
            });
            !context_realm.scene_repo.scenes.is_empty()
        })
    }
}
