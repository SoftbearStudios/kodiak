// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{LockstepContext, LockstepPlayer, LockstepTick};
use crate::bitcode::{self, *};
use crate::{ArenaEntry, CompatHasher, PlayerId};
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};

#[derive(Encode, Decode)]
pub struct Lockstep<W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    pub context: LockstepContext<W>,
    pub world: W,
}

impl<W: LockstepWorld + Clone> Clone for Lockstep<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            world: self.world.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        let Self { context, world } = self;
        context.clone_from(&source.context);
        world.clone_from(&source.world);
    }
}

impl<W: LockstepWorld + Default> Default for Lockstep<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<W: LockstepWorld + Debug> Debug for Lockstep<W>
where
    [(); W::LAG_COMPENSATION]:,
    W::Player: Debug,
    W::Input: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { context, world } = self;
        f.debug_struct("Lockstep")
            .field("context", context)
            .field("world", world)
            .finish()
    }
}

pub trait LockstepWorld: Hash + Debug + Clone + Sized {
    const TPS: usize = 16;
    const TICK_PERIOD_SECS: f32 = 1.0 / (Self::TPS as f32);
    // TODO compress controls better so we can send more within min ethernet packet size of 64 bytes.
    // Before increasing this, make sure max ethernet packet size < 64 bytes so good network
    // conditions don't suffer an overhead.
    const INPUTS_PER_EFFICIENT_PACKET: usize = 6;
    /// 250ms is balance between most games (200ms) and ensuring good experience for high ping.
    const LAG_COMPENSATION: usize = Self::TPS.div_ceil(2) + 2;
    /// Allow the client to predict further than [`LAG_COMPENSATION`] in case latency spikes, but don't
    /// set too high or client could freeze due to too much physics.
    const MAX_PREDICTION: usize = Self::LAG_COMPENSATION + 2;
    /// Buffer size to reduce jitter on server.
    const BUFFERED_TICKS: usize = Self::MAX_PREDICTION;
    /// Do not overwrite.
    const MAX_LATENCY: u8 = Self::LAG_COMPENSATION as u8 - 1;
    /// When server gives update, interpolate from the old prediction to the new one.
    const INTERPOLATE_PREDICTION: bool = false;

    type Player: Hash + Debug + Clone + Encode + DecodeOwned;
    type Input: Copy + Debug + Hash + Default + Encode + DecodeOwned;
    type Info = ();
    type Tick: Clone + Default + Encode + DecodeOwned = ();

    fn tick(
        &mut self,
        _tick: Self::Tick,
        context: &mut LockstepContext<Self>,
        predicting: Option<PlayerId>,
        interpolation_prediction: bool,
        on_info: &mut dyn FnMut(Self::Info),
    ) where
        [(); Self::LAG_COMPENSATION]:;

    /// 0..1
    fn target_buffer(_supports_unreliable: bool) -> f32 {
        0.5
    }

    /// Info to produce if the server overwrites the client state.
    fn on_complete() -> Option<Self::Info> {
        None
    }

    /// t is (0, 1)
    fn lerp(
        &self,
        next: &Self,
        _t: f32,
        _predicting: Option<PlayerId>,
        _interpolation_prediction: bool,
    ) -> Self {
        next.clone()
    }

    /// t is (0, 1)
    fn lerp_player(
        _player: &Self::Player,
        next: &Self::Player,
        _t: f32,
        _interpolation_prediction: bool,
    ) -> Self::Player {
        next.clone()
    }

    fn is_valid(_input: &Self::Input) -> bool {
        true
    }

    fn is_predicted(_info: &Self::Info, _my_id: PlayerId) -> bool {
        false
    }
}

impl<W: LockstepWorld> Lockstep<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    pub(crate) fn new(world: W) -> Self {
        Self {
            context: Default::default(),
            world,
        }
    }

    pub(crate) fn checksum(&self) -> u32 {
        let mut h = CompatHasher::default();
        let Self { context, world } = self;
        context.hash(&mut h);
        world.hash(&mut h);
        h.finish() as u32
    }

    pub(crate) fn tick(
        &mut self,
        tick: LockstepTick<W>,
        predicting: Option<PlayerId>,
        interpolation_prediction: bool,
        on_info: &mut dyn FnMut(W::Info),
    ) {
        self.context.tick_id += 1;
        for (player_id, overwrite) in tick.overwrites {
            if let Some(new) = overwrite {
                match self.context.players.entry(player_id) {
                    ArenaEntry::Vacant(vacant) => {
                        vacant.insert(LockstepPlayer {
                            inner: new,
                            input: Default::default(),
                        });
                    }
                    ArenaEntry::Occupied(occupied) => {
                        occupied.into_mut().inner = new;
                    }
                }
            } else {
                self.context.players.remove(player_id);
            }
        }
        for (player_id, input) in tick.inputs {
            if let Some(player) = self.context.players.get_mut(player_id) {
                player.input = input;
            } else if predicting.is_none() && !interpolation_prediction {
                panic!("missing {player_id:?}");
            }
        }
        self.world.tick(
            tick.inner,
            &mut self.context,
            predicting,
            interpolation_prediction,
            on_info,
        );
    }

    pub fn lerp(
        &self,
        next: &Self,
        t: f32,
        predicting: Option<PlayerId>,
        interpolation_prediction: bool,
    ) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            context: self
                .context
                .lerp(&next.context, t, predicting, interpolation_prediction),
            world: self
                .world
                .lerp(&next.world, t, predicting, interpolation_prediction),
        }
    }
}
