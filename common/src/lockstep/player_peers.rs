use super::{LockstepPlayer, LockstepWorld};
use crate::{ArenaKey, ArenaMap};
use plasma_protocol::PlayerId;

/// All players except one, useful for player vs. player interactions.
pub struct LockstepPeers<'a, W: LockstepWorld>
where
    [(); W::LAG_COMPENSATION]:,
{
    slices: [&'a mut [Option<LockstepPlayer<W>>]; 2], // first: &[_], gap: &_, second: &[_]
}

impl<'a, W: LockstepWorld> LockstepPeers<'a, W>
where
    [(); W::LAG_COMPENSATION]:,
{
    /// First return value represents all players except `player_id`.
    ///
    /// Second return value represents `player_id`, if such a player exists.
    pub fn new(
        players: &'a mut ArenaMap<PlayerId, LockstepPlayer<W>>,
        player_id: PlayerId,
    ) -> (Self, Option<&'a mut LockstepPlayer<W>>) {
        let slots = players.raw_slots();
        let (first, gap_second) = slots.split_at_mut(player_id.to_index());
        let (gap, second) = gap_second.split_at_mut(1);
        let player = gap[0].as_mut();
        (
            Self {
                slices: [first, second],
            },
            player,
        )
    }

    /// Iterate all players except one.
    pub fn iter(&self) -> impl Iterator<Item = (PlayerId, &LockstepPlayer<W>)> {
        // Plus one for gap between them.
        let offsets = [0, self.slices[0].len() + 1];

        self.slices.iter().zip(offsets).flat_map(|(slice, offset)| {
            slice
                .iter()
                .enumerate()
                .filter_map(move |(i, p)| Some(ArenaKey::from_index(i + offset)).zip(p.as_ref()))
        })
    }

    /// Iterate all players except one.
    pub fn iter_mut<'b: 'a>(
        &'b mut self,
    ) -> impl Iterator<Item = (PlayerId, &mut LockstepPlayer<W>)> {
        // Plus one for gap between them.
        let offsets = [0, self.slices[0].len() + 1];

        self.slices
            .iter_mut()
            .zip(offsets)
            .flat_map(|(slice, offset)| {
                slice.iter_mut().enumerate().filter_map(move |(i, p)| {
                    Some(ArenaKey::from_index(i + offset)).zip(p.as_mut())
                })
            })
    }
}
