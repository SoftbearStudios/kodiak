// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::phase::LockstepPhase;
use super::{LockstepPlayer, LockstepWorld};
use crate::bitcode::{self, *};
use crate::{ArenaMap, PlayerId};
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};

/// The context is the part of the world defined by the lockstep model.
#[derive(Clone, Encode, Decode)]
pub struct LockstepContext<W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    pub tick_id: u32,
    pub players: ArenaMap<PlayerId, LockstepPlayer<W>>,
}

impl<W: LockstepWorld> Default for LockstepContext<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn default() -> Self {
        Self {
            tick_id: Default::default(),
            players: Default::default(),
        }
    }
}

impl<W: LockstepWorld> Debug for LockstepContext<W>
where
    [(); W::LAG_COMPENSATION]:,
    W::Player: Debug,
    W::Input: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self { tick_id, players } = self;
        f.debug_struct("LockstepContext")
            .field("tick_id", tick_id)
            .field("players", players)
            .finish()
    }
}

impl<W: LockstepWorld> Hash for LockstepContext<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        let Self { tick_id, players } = self;
        tick_id.hash(state);
        players.hash(state);
    }
}

impl<W: LockstepWorld> LockstepContext<W>
where
    [(); W::LAG_COMPENSATION]:,
{
    /// t must be [0,1].
    ///
    /// Resulting set of player ids will match `next`.
    pub(crate) fn lerp(&self, next: &Self, t: f32, phase: &LockstepPhase) -> Self {
        let mut players = next.players.clone();
        for (player_id, next) in players.iter_mut() {
            /*
            if predicting == Some(player_id) && !interpolation_prediction {
                // Lerping is only necessary when inputs are not predicted. Here, it would be
                // harmful for unknown reasons.
                continue;
            }
            */
            if let Some(prev) = self.players.get(player_id) {
                let lerped = W::lerp_player(player_id, prev, &*next, t, phase);
                next.inner = lerped;
            }
        }
        Self {
            tick_id: next.tick_id,
            players,
        }
    }
}
