// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::service::{ArenaService, Player, PlayerRepo};
use crate::{
    unwrap_or_return, ArenaId, InvitationDto, InvitationId, InvitationRequest, InvitationUpdate,
    PlayerId, RealmId, ServerId,
};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Invitations, shared by all arenas.
pub struct InvitationRepo<G: ArenaService> {
    // TODO: Prune.
    invitations: HashMap<InvitationId, Invitation>,
    _spooky: PhantomData<G>,
}

/// For routing invitations.
#[derive(Clone, Debug)]
pub struct Invitation {
    /// Sender arena id.
    pub arena_id: ArenaId,
    /// Sender (none for entire arenas e.g. temporary realms).
    pub player_id: Option<PlayerId>,
}

/// Invitation related data stored in player.
#[derive(Debug, Default)]
pub struct ClientInvitationData {
    /// Incoming invitation accepted by player.
    pub invitation_accepted: Option<InvitationDto>,
    /// Outgoing invitation created by player.
    pub invitation_created: Option<InvitationId>,
}

impl ClientInvitationData {
    pub(crate) fn initializer(&self) -> Option<InvitationUpdate> {
        self.invitation_accepted
            .map(|dto| Some(dto.invitation_id))
            .map(InvitationUpdate::Accepted)
    }
}

impl<G: ArenaService> Default for InvitationRepo<G> {
    fn default() -> Self {
        Self {
            invitations: HashMap::new(),
            _spooky: PhantomData,
        }
    }
}

impl<G: ArenaService> InvitationRepo<G> {
    /// Looks up an invitation by id.
    #[allow(unused)]
    pub fn get(&self, invitation_id: InvitationId) -> Option<&Invitation> {
        self.invitations.get(&invitation_id)
    }

    /// Returns how many invitations are cached.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.invitations.len()
    }

    pub(crate) fn prepare_temporary_invitation(
        &mut self,
        server_id: ServerId,
    ) -> Option<InvitationId> {
        let mut governor = u8::MAX;
        let mut invitation_id = InvitationId::generate(server_id.number);
        loop {
            if let Entry::Vacant(entry) = self.invitations.entry(invitation_id) {
                entry.insert(Invitation {
                    arena_id: ArenaId {
                        realm_id: RealmId::Temporary(invitation_id),
                        scene_id: Default::default(),
                    },
                    player_id: None,
                });
                break Some(invitation_id);
            }
            governor = governor.checked_sub(1)?;
            invitation_id = InvitationId::generate(server_id.number);
        }
    }

    /// Forgets any invitation the player created.
    pub(crate) fn forget_player_invitation(&mut self, player: &mut Player<G>) {
        let client = unwrap_or_return!(player.client_mut());
        if let Some(invitation_id) = client.invitation.invitation_created {
            let removed = self.invitations.remove(&invitation_id);
            debug_assert!(removed.is_some(), "invitation was cleared elsewhere");
            client.invitation.invitation_created = None;
        }
    }

    /// Forgets all invitations to the given arena. Inefficient.
    pub(crate) fn forget_arena_invitations(&mut self, arena_id: ArenaId) {
        self.invitations.retain(|_, i| i.arena_id != arena_id);
    }

    pub(crate) fn accept(
        &self,
        req_player_id: PlayerId,
        invitation_id: Option<InvitationId>,
        players: &mut PlayerRepo<G>,
    ) -> Result<InvitationUpdate, &'static str> {
        let req_player = players
            .get_mut(req_player_id)
            .ok_or("req player doesn't exist")?;

        let req_client = req_player
            .client_mut()
            .ok_or("only clients can accept invitations")?;

        req_client.invitation.invitation_accepted = invitation_id.and_then(|invitation_id| {
            self.invitations.get(&invitation_id).and_then(|invitation| {
                invitation.player_id.map(|player_id| InvitationDto {
                    player_id,
                    invitation_id,
                })
            })
        });
        req_client.metrics.invited |= req_client.invitation.invitation_accepted.is_some();

        // println!("@@@@@@@ ACCEPTED {:?} = {:?}", invitation_id, req_client.invitation.invitation_accepted);

        if invitation_id.is_none() || req_client.invitation.invitation_accepted.is_some() {
            Ok(InvitationUpdate::Accepted(
                req_client
                    .invitation
                    .invitation_accepted
                    .map(|d| d.invitation_id),
            ))
        } else {
            Err("no such invitation")
        }
    }

    /// Requests an invitation id (new or recycled).
    fn create(
        &mut self,
        req_player_id: PlayerId,
        arena_id: ArenaId,
        server_id: ServerId,
        players: &mut PlayerRepo<G>,
    ) -> Result<InvitationUpdate, &'static str> {
        let req_player = players
            .get_mut(req_player_id)
            .ok_or("req player doesn't exist")?;

        let req_client = req_player
            .client_mut()
            .ok_or("only clients can request invitations")?;

        // Silently ignore case of previously created invitation id.
        let invitation_id = if let Some(invitation_id) = req_client.invitation.invitation_created {
            invitation_id
        } else {
            let mut governor = u8::MAX;
            loop {
                let invitation_id = InvitationId::generate(server_id.number);
                if let Entry::Vacant(entry) = self.invitations.entry(invitation_id) {
                    entry.insert(Invitation {
                        arena_id,
                        player_id: Some(req_player_id),
                    });
                    req_client.invitation.invitation_created = Some(invitation_id);
                    break invitation_id;
                }
                if let Some(new) = governor.checked_sub(1) {
                    governor = new;
                } else {
                    return Err("could not create invitation id");
                }
            }
        };

        Ok(InvitationUpdate::Created(invitation_id))
    }

    pub fn handle_invitation_request(
        &mut self,
        player_id: PlayerId,
        request: InvitationRequest,
        arena_id: ArenaId,
        server_id: ServerId,
        players: &mut PlayerRepo<G>,
    ) -> Result<InvitationUpdate, &'static str> {
        match request {
            InvitationRequest::Accept(invitation_id) => {
                self.accept(player_id, invitation_id, players)
            }
            InvitationRequest::Create => self.create(player_id, arena_id, server_id, players),
        }
    }
}
