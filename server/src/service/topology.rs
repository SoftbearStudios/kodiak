// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    Angle, ArenaId, Cardinal4, InstanceNumber, RegionId, SceneId, ServerId, ServerUseTopology,
    TierNumber,
};
use kodiak_common::rand::prelude::SliceRandom;
use kodiak_common::rand::thread_rng;
use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;

#[derive(Debug)]
pub struct Topology {
    pub local_server_id: ServerId,
    pub local_arena_id: ArenaId,
    /// For lag calculations.
    pub regions: HashMap<ServerId, RegionId>,
    pub tiers: BTreeMap<Option<TierNumber>, Tier>,
}

#[derive(Default, Debug)]
pub struct Tier {
    pub arenas: BTreeMap<ServerId, Arena>,
}

#[derive(Debug, Default)]
pub struct Arena {
    pub player_count: u16,
    pub instance_numbers: Vec<InstanceNumber>,
}

impl Topology {
    pub(crate) fn new(local_server_id: ServerId, local_arena_id: ArenaId) -> Self {
        Self {
            local_server_id,
            local_arena_id,
            regions: Default::default(),
            tiers: Default::default(),
        }
    }

    pub fn region_id(&self, server_id: ServerId) -> Option<RegionId> {
        self.regions.get(&server_id).copied()
    }

    pub(crate) fn update(&mut self, servers: &HashMap<ServerId, ServerUseTopology>) {
        self.regions.clear();
        self.tiers.clear();
        for (&server_id, server) in servers {
            self.regions.insert(server_id, server.region_id);
            let Some(realm) = server.realm(self.local_arena_id.realm_id) else {
                continue;
            };
            for (scene_id, arena) in &realm.scenes {
                let tier = self.tiers.entry(scene_id.tier_number).or_default();
                let server = tier.arenas.entry(server_id).or_default();
                server.player_count += arena.player_count;
                server.instance_numbers.push(scene_id.instance_number);
            }
        }
    }

    pub fn max_tier_number(&self) -> Option<TierNumber> {
        self.max_sanctioned_tier_number()
            .max(self.local_arena_id.scene_id.tier_number)
    }

    pub fn max_sanctioned_tier_number(&self) -> Option<TierNumber> {
        self.tiers.last_key_value().and_then(|(n, _)| *n)
    }

    pub fn tier_count(&self) -> usize {
        self.max_tier_number()
            .map(|n| n.0.get() as usize)
            .unwrap_or(0)
            + 1
    }

    pub fn next_angle(&self, angle: Angle) -> Option<(ServerId, ArenaId)> {
        self.next_cardinal_4(angle.to_cardinal_4())
    }

    pub fn next_cardinal_4(&self, cardinal: Cardinal4) -> Option<(ServerId, ArenaId)> {
        match cardinal {
            Cardinal4::North => self.next_higher(),
            Cardinal4::East => self.next_right(),
            Cardinal4::South => self.next_lower(),
            Cardinal4::West => self.next_left(),
        }
    }

    pub fn next_left(&self) -> Option<(ServerId, ArenaId)> {
        self.next_horizontal(false)
    }

    pub fn next_right(&self) -> Option<(ServerId, ArenaId)> {
        self.next_horizontal(true)
    }

    // TODO: by not considering instace, much of world is not reachable; and server switching may end up on nonexistent instance.
    fn next_horizontal(&self, right: bool) -> Option<(ServerId, ArenaId)> {
        let current_tier = self.tiers.get(&self.local_arena_id.scene_id.tier_number)?;
        if current_tier.arenas.len() == 0 {
            debug_assert!(false);
            return None;
        }
        let next = if right {
            current_tier
                .arenas
                .range((Bound::Excluded(self.local_server_id), Bound::Unbounded))
                .next()
        } else {
            current_tier
                .arenas
                .range((Bound::Unbounded, Bound::Excluded(self.local_server_id)))
                .next_back()
        };
        let (&next_server_id, next_arena) = if let Some(next) = next {
            next
        } else if right {
            current_tier.arenas.iter().next().unwrap()
        } else {
            current_tier.arenas.iter().next_back().unwrap()
        };
        Some((
            next_server_id,
            ArenaId::new(
                self.local_arena_id.realm_id,
                SceneId::new(
                    self.local_arena_id.scene_id.tier_number,
                    next_arena
                        .instance_numbers
                        .choose(&mut thread_rng())
                        .cloned()
                        .unwrap_or_default(),
                ),
            ),
        ))
        .filter(|_| next_server_id != self.local_server_id)
    }

    pub fn next_higher(&self) -> Option<(ServerId, ArenaId)> {
        self.next_vertical(true)
    }

    pub fn next_lower(&self) -> Option<(ServerId, ArenaId)> {
        self.next_vertical(false)
    }

    fn next_vertical(&self, higher: bool) -> Option<(ServerId, ArenaId)> {
        let (&next_tier_number, next_tier) = if higher {
            self.tiers
                .range((
                    Bound::Excluded(self.local_arena_id.scene_id.tier_number),
                    Bound::Unbounded,
                ))
                .next()?
        } else {
            self.tiers
                .range((
                    Bound::Unbounded,
                    Bound::Excluded(self.local_arena_id.scene_id.tier_number),
                ))
                .next_back()?
        };

        let Some(current_tier) = self.tiers.get(&self.local_arena_id.scene_id.tier_number) else {
            return next_tier.arenas.iter().next().map(|(&server_id, arena)| {
                (
                    server_id,
                    ArenaId::new(
                        self.local_arena_id.realm_id,
                        SceneId::new(
                            next_tier_number,
                            arena
                                .instance_numbers
                                .choose(&mut thread_rng())
                                .cloned()
                                .unwrap_or_default(),
                        ),
                    ),
                )
            });
        };
        let current_index = current_tier
            .arenas
            .range((Bound::Unbounded, Bound::Excluded(self.local_server_id)))
            .count();
        let current_total = current_tier.arenas.len();
        if current_total == 0 {
            debug_assert!(false);
            return None;
        }
        let horizontal = (current_index) as f32 / current_total.max(1) as f32;
        let next_total = next_tier.arenas.len();
        if next_total == 0 {
            debug_assert!(false);
            return None;
        }
        let next_index = (horizontal * next_total as f32).round();
        let left_choice = (next_index.floor() as usize).clamp(0, next_total - 1);
        let right_choice = (next_index.ceil() as usize).clamp(0, next_total - 1);
        let choice = if next_tier
            .arenas
            .iter()
            .nth(left_choice)
            .unwrap()
            .1
            .player_count
            < next_tier
                .arenas
                .iter()
                .nth(right_choice)
                .unwrap()
                .1
                .player_count
        {
            left_choice
        } else {
            right_choice
        };
        let (&server_id, arena) = next_tier.arenas.iter().nth(choice).unwrap();
        Some((
            server_id,
            ArenaId::new(
                self.local_arena_id.realm_id,
                SceneId::new(
                    next_tier_number,
                    arena
                        .instance_numbers
                        .choose(&mut thread_rng())
                        .cloned()
                        .unwrap(),
                ),
            ),
        ))
    }
}
