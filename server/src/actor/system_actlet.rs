// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::ServerActor;
use crate::service::ArenaService;
use crate::util::diff_small_n;
use crate::{
    InstancePickerDto, RegionId, SceneId, ServerId, SystemQuery, SystemResponse, SystemUpdate,
    UserAgentId,
};
use actix::{Handler, Message};
use kodiak_common::rand::{thread_rng, Rng};
use kodiak_common::DomainName;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

/// Monitors web servers and changes DNS to recover from servers going offline.
///
/// System, in this case, refers to a distributed system of multiple servers.
pub struct SystemActlet<G: ArenaService> {
    /// Compatible instances. For diffing.
    previous: Arc<[InstancePickerDto]>,
    /// All compatible instances on the domain, from plasma.
    pub(crate) instances: Box<[InstancePickerDto]>,
    pub(crate) servers: Box<[ServerPickerItem]>,
    pub(crate) available_servers: Arc<[ServerId]>,
    pub(crate) alternative_domains: Arc<[DomainName]>,
    _spooky: PhantomData<G>,
}

pub struct ServerPickerItem {
    pub(crate) server_id: ServerId,
    // Sanctioned tier numbers.
    // pub(crate) tier_numbers: HashSet<Option<TierNumber>>,
    pub(crate) datacenter: String,
    pub(crate) region_id: RegionId,
    /// Total load (all arenas).
    pub(crate) player_count: u32,
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
struct ServerPriority {
    /// Client asked for a specific server, but not this one.
    wrong_server_id: bool,
    /// Client asked for a tier number that this server doesn't support.
    //unsupported_tier_number: bool,
    distance: u8,
    load: u32,
}

impl<G: ArenaService> SystemActlet<G> {
    pub(crate) fn new() -> Self {
        Self {
            previous: Vec::new().into(),
            instances: Vec::new().into(),
            servers: Vec::new().into(),
            available_servers: Vec::new().into(),
            alternative_domains: Vec::new().into(),
            _spooky: PhantomData,
        }
    }

    pub(crate) fn initializer(&self) -> Option<SystemUpdate> {
        (!self.previous.is_empty()).then(|| SystemUpdate::Added(Arc::clone(&self.previous)))
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn delta(
        &mut self,
    ) -> Option<(Arc<[InstancePickerDto]>, Arc<[(ServerId, SceneId)]>)> {
        if let Some((added, removed)) = diff_small_n(&self.previous, &self.instances, |dto| {
            (dto.server_id, dto.scene_id)
        }) {
            self.previous = self.instances.iter().cloned().collect();
            Some((added.into(), removed.into()))
        } else {
            None
        }
    }

    /// Iterates available servers, their absolute priorities (lower is higher priority),
    /// and player counts, in an undefined order.
    fn iter_server_priorities<'a>(
        &'a self,
        request: &'a SystemRequest,
        filter: Option<ServerId>,
        self_server_id: ServerId,
    ) -> impl Iterator<Item = (ServerId, ServerPriority)> + 'a {
        let mut rng = thread_rng();
        let self_datacenter = self
            .servers
            .iter()
            .find(|s| s.server_id == self_server_id)
            .map(|s| &s.datacenter);
        self.servers
            .iter()
            .filter(move |s| filter.map(move |f| s.server_id == f).unwrap_or(true))
            .map(move |server| {
                (
                    server.server_id,
                    ServerPriority {
                        wrong_server_id: Some(server.server_id) != request.server_id,
                        /*
                        unsupported_tier_number: !server
                            .tier_numbers
                            .contains(&request.tier_number),
                        */
                        distance: if G::GAME_CONSTANTS.geodns_enabled {
                            // Assume GeoDNS worked.
                            (Some(&server.datacenter) != self_datacenter) as u8
                        } else {
                            request
                                .region_id
                                .map(|region_id| region_id.distance(server.region_id))
                                .unwrap_or(0)
                        },
                        load: server.player_count.saturating_add(rng.gen_range(0..5)),
                    },
                )
            })
    }
}

/// Asks the server about the distributed system of servers.
#[derive(Message)]
#[rtype(result = "SystemResponse")]
pub struct SystemRequest {
    pub(crate) query: SystemQuery,
    /// [`RegionId`] preference.
    pub(crate) region_id: Option<RegionId>,
    pub(crate) user_agent_id: Option<UserAgentId>,
}

impl Deref for SystemRequest {
    type Target = SystemQuery;

    fn deref(&self) -> &Self::Target {
        &self.query
    }
}

/// Reports whether infrastructure is healthy (hardware and actor are running properly).
impl<G: ArenaService> Handler<SystemRequest> for ServerActor<G> {
    type Result = SystemResponse;

    fn handle(&mut self, request: SystemRequest, ctx: &mut Self::Context) -> Self::Result {
        /*
        self.plasma
                    .servers
                    .iter()
                    .find_map(|(server_id, server_dto)| {
                        server_dto
                            .other_realms
                            .contains_key(&realm_id)
                            .then_some((*server_id, None))
                    })
        */

        /*
        println!(
            "@@@ {:?}",
            SystemActlet::iter_server_priorities(
                &self.system,
                &request,
                self.plasma.role.is_unlisted().then_some(self.server_id)
            )
            .collect::<Vec<_>>()
        );
        */

        // TODO: named realm support.
        let ideal_server_id = if request
            .arena_id
            .realm_id()
            .unwrap_or_default()
            .is_public_default()
        {
            SystemActlet::iter_server_priorities(
                &self.system,
                &request,
                self.plasma.role.is_unlisted().then_some(self.server_id),
                self.server_id,
            )
            .min_by_key(|&(_, priority)| priority)
            .map(|(s, _)| s)
            .unwrap_or(self.server_id)
        } else {
            // E.g. temporary.
            request.server_id.unwrap_or(self.server_id)
        };

        SystemResponse {
            server_id: ideal_server_id,
            languages: self.translations.languages.clone(),
            snippets: self.clients.get_snippets(
                request.referrer,
                request.cohort_id,
                request.region_id,
                request.user_agent_id,
            ),
            available_servers: self.system.available_servers.clone(),
            alternative_domains: self.system.alternative_domains.clone(),
            translation: self.handle(request.query.translation, ctx),
        }
    }
}
