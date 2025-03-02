// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::arena_context::SendPlasmaRequest;
use super::{ChatRepo, ShardContextProvider};
use crate::actor::{ClientActlet, PlasmaActlet, SystemActlet};
use crate::service::{
    ArenaContext, ArenaService, InvitationRepo, LeaderboardRepo, LiveboardRepo, MetricRepo,
};
use crate::{ArenaId, InstancePickerDto, PlayerId, ReconnectionToken, SceneId, ServerId};
use std::collections::HashMap;
use std::sync::Arc;

// TODO: was pub(crate)
pub struct SceneRepo<G: ArenaService> {
    pub(crate) scenes: HashMap<SceneId, Scene<G>>,
}

impl<G: ArenaService> SceneRepo<G> {
    #[allow(unused)]
    pub(crate) fn get(&self, scene_id: &SceneId) -> Option<&Scene<G>> {
        self.scenes.get(scene_id)
    }

    pub(crate) fn get_mut(&mut self, scene_id: SceneId) -> Option<&mut Scene<G>> {
        self.scenes.get_mut(&scene_id)
    }

    #[allow(unused)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (SceneId, &Scene<G>)> {
        self.scenes.iter().map(|(k, v)| (*k, v))
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (SceneId, &mut Scene<G>)> {
        self.scenes.iter_mut().map(|(k, v)| (*k, v))
    }
}

impl<G: ArenaService> Default for SceneRepo<G> {
    fn default() -> Self {
        Self {
            scenes: Default::default(),
        }
    }
}

pub struct Scene<G: ArenaService> {
    pub(crate) arena: Arena<G>,
    pub(crate) per_scene: <G::Shard as ShardContextProvider<G>>::PerScene,
}

impl<G: ArenaService> Scene<G> {
    pub(crate) fn can_reconnect(&self, (player_id, token): (PlayerId, ReconnectionToken)) -> bool {
        self.arena
            .arena_context
            .players
            .get(player_id)
            .and_then(|p| p.client().map(|c| c.token == token))
            .unwrap_or(false)
    }
}

/// Contains a [`ArenaService`] and the corresponding [`Context`].
pub struct Arena<G: ArenaService> {
    pub arena_context: ArenaContext<G>,
    pub arena_service: G,
}

impl<G: ArenaService> Arena<G> {
    pub(crate) fn new(
        server_id: ServerId,
        arena_id: ArenaId,
        send_plasma_request: SendPlasmaRequest,
    ) -> Self {
        let mut arena_context = ArenaContext::new(server_id, arena_id, send_plasma_request);
        Self {
            arena_service: G::new(&mut arena_context),
            arena_context,
        }
    }

    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    pub(crate) fn update(
        &mut self,
        clients: &mut ClientActlet<G>,
        liveboard: &mut LiveboardRepo<G>,
        leaderboard: &LeaderboardRepo<G>,
        invitations: &mut InvitationRepo<G>,
        chat: &mut ChatRepo<G>,
        metrics: &mut MetricRepo<G>,
        server_delta: &Option<(Arc<[InstancePickerDto]>, Arc<[(ServerId, SceneId)]>)>,
        players_online: u32,
        server_id: ServerId,
        arena_id: ArenaId,
        plasma: &PlasmaActlet,
        system: &SystemActlet<G>,
        temporaries_available: bool,
    ) {
        // Spawn/de-spawn clients and bots.
        clients.prune(
            &mut self.arena_service,
            &mut self.arena_context,
            invitations,
            metrics,
            arena_id,
        );
        self.arena_context.bots.update_count(
            &mut self.arena_service,
            &mut self.arena_context.players,
            &self.arena_context.settings.engine,
        );

        // Update game logic.
        self.arena_context.topology.update(&plasma.servers);
        self.arena_context.send_to_plasma.web_socket = plasma.web_socket.sender.clone();
        self.arena_service.tick(&mut self.arena_context);
        let annoucements = self.arena_context.players.update_is_alive_and_team_id(
            &mut self.arena_service,
            metrics,
            server_id,
            arena_id,
        );
        for announcement in annoucements {
            chat.broadcast_message(announcement, None, std::iter::once(&mut *self), None, false);
        }

        // Update clients and bots.
        clients.update(
            &self.arena_service,
            &mut self.arena_context.players,
            liveboard,
            leaderboard,
            server_delta,
            players_online,
            server_id,
            arena_id,
            plasma,
            system,
            temporaries_available,
        );
        self.arena_context.bots.update(
            &self.arena_service,
            &mut self.arena_context.players,
            &self.arena_context.settings,
        );

        liveboard.process(
            arena_id.scene_id,
            &self.arena_service,
            &self.arena_context.players,
        );

        // Post-update game logic.
        self.arena_service.post_update(&mut self.arena_context);

        // Bot commands/joining/leaving, postponed because no commands should be issued between
        // `GameService::tick` and `GameService::post_update`.
        self.arena_context
            .bots
            .post_update(&mut self.arena_service, &mut self.arena_context.players);
    }
}
