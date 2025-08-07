// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::phase::{LockstepPhase, LockstepPhaseInner};
use super::{
    Lockstep, LockstepClientData, LockstepInput, LockstepRequest, LockstepTick, LockstepUpdate,
    LockstepWorld,
};
use crate::{ArenaKey, ArenaMap, PlayerId};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;

/// Implements lockstep model on the game server.
pub struct LockstepServer<W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    /// Most recent server state, i.e. the authoritative state.
    pub real: Lockstep<W>,
    /// Pending inputs not yet applied to `real`
    pub current: LockstepTick<W>,
}

impl<W: LockstepWorld + Default> Default for LockstepServer<W>
where
    [(); W::LAG_COMPENSATION]:,
    [(); W::MAX_PREDICTION]:,
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
    [(); W::BUFFERED_TICKS]:,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<W: LockstepWorld> Deref for LockstepServer<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    type Target = Lockstep<W>;

    fn deref(&self) -> &Self::Target {
        &self.real
    }
}

impl<W: LockstepWorld + Debug> Debug for LockstepServer<W>
where
    [(); W::LAG_COMPENSATION]:,
    W::Player: Debug,
    W::Input: Debug,
    W::Tick: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { real, current } = self;
        f.debug_struct("LockstepServer")
            .field("real", real)
            .field("current", current)
            .finish()
    }
}

pub fn lockstep_get<'a, K: ArenaKey + Ord, V1, V2>(
    map: &'a ArenaMap<K, V1>,
    convert: impl FnOnce(&V1) -> &V2,
    overwrites: &'a BTreeMap<K, Option<V2>>,
    key: K,
) -> Option<&'a V2> {
    overwrites
        .get(&key)
        .map(|o| o.as_ref())
        .unwrap_or_else(move || map.get(key).map(convert))
}

pub fn lockstep_mut<'a, K: ArenaKey + Ord, V1, V2>(
    map: &'a ArenaMap<K, V1>,
    convert: impl FnOnce(&V1) -> V2,
    overwrites: &'a mut BTreeMap<K, Option<V2>>,
    key: K,
) -> &'a mut Option<V2> {
    let entry = overwrites.entry(key);
    match entry {
        Entry::Vacant(vacant) => vacant.insert(map.get(key).map(convert)),
        Entry::Occupied(occupied) => occupied.into_mut(),
    }
}

impl<W: LockstepWorld> LockstepServer<W>
where
    [(); W::LAG_COMPENSATION]:,
    [(); W::MAX_PREDICTION]:,
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
    [(); W::BUFFERED_TICKS]:,
{
    pub fn new(world: W) -> Self {
        Self {
            real: Lockstep::new(world),
            current: Default::default(),
        }
    }

    pub fn player(&self, player_id: PlayerId) -> Option<&W::Player> {
        lockstep_get(
            &self.real.context.players,
            |p| &p.inner,
            &self.current.overwrites,
            player_id,
        )
    }

    pub fn player_mut(&mut self, player_id: PlayerId) -> &mut Option<W::Player> {
        lockstep_mut(
            &self.real.context.players,
            |p| p.inner.clone(),
            &mut self.current.overwrites,
            player_id,
        )
    }

    pub fn existing_player_mut(&mut self, player_id: PlayerId) -> Option<&mut W::Player> {
        self.player(player_id)?;
        self.player_mut(player_id).as_mut()
    }

    /// Removes a player and cleans up any pending input.
    pub fn player_left(&mut self, player_id: PlayerId) {
        debug_assert!(self.player(player_id).is_some(), "{player_id:?}");
        *self.player_mut(player_id) = None;
        self.current.inputs.remove(player_id);
    }

    pub fn request(
        &mut self,
        player_id: PlayerId,
        request: LockstepRequest<W>,
        client_data: Option<&mut LockstepClientData<W>>,
        supports_unreliable: bool,
    ) {
        let reliable = !supports_unreliable;
        if let Some(client) = client_data {
            //println!("received {} init={} last={}", request.inputs.last_input_id, client.initialized, client.last_applied_command_id);
            if !client.initialized {
                // Likely situation: client switched to this server but sent a message intended
                // for the old server.
                //
                // Note: Initialization known to server doesn't directly stop client from
                // sending messages, but it implies that the new connection is known to the
                // client already.
                return;
            }
            for input in request.inputs.into_input_iter() {
                if !W::is_valid(&input.inner) {
                    debug_assert!(
                        false,
                        "received invalid input {player_id:?}: {:?}",
                        input.inner
                    );
                    continue;
                }

                // We can't keep buffering forever.
                if client.receive_buffer.is_full() {
                    #[cfg(feature = "log")]
                    log::info!("receive buffer full {player_id:?}");
                    return;
                }

                // We've already applied these commands.
                if input.input_id <= client.last_applied_command_id {
                    debug_assert!(
                        !reliable,
                        "received stale command {} <= {} for {player_id:?}",
                        input.input_id, client.last_applied_command_id
                    );
                    continue;
                }

                // Assuming client is sending unreliable messages we have to reorder them to apply in correct order.
                let Err(index) = client
                    .receive_buffer
                    .binary_search_by(|v| v.input_id.cmp(&input.input_id))
                else {
                    debug_assert!(!reliable, "received duplicate command {player_id:?}");
                    continue;
                };
                client.last_received_command_id =
                    client.last_received_command_id.max(input.input_id);
                client.receive_buffer.insert(index, input);
            }
        } else if self.player(player_id).is_some() {
            // Bots always send exactly 1 Controls.
            self.current
                .inputs
                .insert(player_id, request.inputs.sliding_window[0]);
        }
    }

    pub fn update<'a>(
        &mut self,
        clients: impl Iterator<Item = (PlayerId, &'a mut LockstepClientData<W>)>,
    ) where
        W: 'a,
    {
        for (player_id, client) in clients {
            if self.player(player_id).is_none() {
                continue;
            }
            // Server shouldn't accept stale or duplicate commands.
            debug_assert!(client
                .receive_buffer
                .iter()
                .all(|c| c.input_id > client.last_applied_command_id));
            debug_assert!(client.receive_buffer.is_sorted_by_key(|c| c.input_id));

            if client.receive_buffer.is_empty() {
                continue;
            }
            let LockstepInput { input_id, inner } = client.receive_buffer.remove(0);
            client.last_applied_command_id = input_id;

            self.current.inputs.insert(player_id, inner);
        }

        self.current.checksum = Some(self.checksum());
        #[cfg(feature = "desync")]
        {
            self.current.complete = Some(self.real.clone());
        }
    }

    pub fn client_update(
        &self,
        player_id: PlayerId,
        client_data: &mut LockstepClientData<W>,
    ) -> LockstepUpdate<W> {
        assert!(player_id.is_client());
        let initialize = !std::mem::replace(&mut client_data.initialized, true);
        LockstepUpdate {
            initialization: initialize.then(|| {
                client_data.last_applied_command_id = 0;
                // New.
                client_data.last_received_command_id = 0;
                (player_id, self.real.clone())
            }),
            last_applied_input_id: client_data.last_applied_command_id,
            last_received_input_id: client_data.last_received_command_id,
            tick: self.current.clone(),
            buffered_inputs: client_data.receive_buffer.len(),
        }
    }

    pub fn post_update(&mut self, on_info: &mut dyn FnMut(W::Info)) {
        self.real.tick(
            std::mem::take(&mut self.current),
            &LockstepPhase {
                inner: LockstepPhaseInner::GroundTruth,
            },
            on_info,
        );
    }
}
