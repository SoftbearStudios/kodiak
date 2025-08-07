// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::service::{ArenaService, Bot, BotAction, Player, PlayerInner, PlayerRepo};
use crate::{ArenaSettingsDto, EngineArenaSettings, PlayerAlias, PlayerId};
use kodiak_common::rand::prelude::IteratorRandom;
use kodiak_common::rand::seq::SliceRandom;
use kodiak_common::rand::thread_rng;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::LazyLock;

static BOT_NAMES: LazyLock<Vec<PlayerAlias>> = LazyLock::new(|| {
    include_str!("./bot_names.txt")
        .split('\n')
        .filter(|s| !s.is_empty() && s.len() <= PlayerAlias::capacity())
        .map(PlayerAlias::new_unsanitized)
        .collect()
});

pub fn random_emoji_bot_name() -> PlayerAlias {
    let names = &BOT_NAMES;
    let alias = *names
        .iter()
        .filter(|n| n.len() <= 7)
        .choose(&mut thread_rng())
        .unwrap();
    PlayerAlias::new_unsanitized(&format!("🤖 {alias}"))
}

pub fn random_bot_name() -> PlayerAlias {
    let names = &BOT_NAMES;
    *names.choose(&mut thread_rng()).unwrap()
}

/// Data stored per bot.
#[derive(Debug)]
pub struct PlayerBotData<G: ArenaService> {
    /// Only Some during an update cycle.
    action_buffer: BotAction<G::GameRequest>,
    pub bot: G::Bot,
}

impl<G: ArenaService> Deref for PlayerBotData<G> {
    type Target = G::Bot;

    fn deref(&self) -> &Self::Target {
        &self.bot
    }
}

impl<G: ArenaService> DerefMut for PlayerBotData<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bot
    }
}

impl<G: ArenaService> Default for PlayerBotData<G> {
    fn default() -> Self {
        Self {
            bot: G::Bot::default(),
            action_buffer: BotAction::default(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BotOptions {
    /// Minimum number of bots (always less than or equal to max_bots).
    pub min_bots: usize,
    /// Maximum number of bots.
    pub max_bots: usize,
    /// This percent of real players will help determine the target bot quantity.
    pub bot_percent: isize,
}

/// Manages the storage and updating of bots.
pub struct BotRepo<G: ArenaService> {
    /// Current number of bots.
    pub(crate) count: usize,
    _spooky: PhantomData<G>,
}

impl<G: ArenaService> Default for BotRepo<G> {
    fn default() -> Self {
        Self {
            count: 0,
            _spooky: PhantomData,
        }
    }
}

impl<G: ArenaService> BotRepo<G> {
    /// Updates all bots.
    pub(crate) fn update(
        &mut self,
        service: &G,
        players: &mut PlayerRepo<G>,
        settings: &ArenaSettingsDto<G::ArenaSettings>,
    ) {
        for i in 0..self.count {
            let player_id = PlayerId::nth_bot(i).unwrap();
            let player = players.get_mut(player_id).unwrap();
            let action = if player.regulator.active() {
                G::Bot::update(service, player_id, player, settings)
            } else {
                BotAction::None("inactive")
            };
            player.inner.bot_mut().unwrap().action_buffer = action;
        }
    }

    /// Call after `GameService::post_update` to avoid sending commands between `GameService::tick` and it.
    pub(crate) fn post_update(&mut self, service: &mut G, players: &mut PlayerRepo<G>) {
        for i in 0..self.count {
            let player_id = PlayerId::nth_bot(i).unwrap();
            let player = players.get_mut(player_id).unwrap();

            match std::mem::take(&mut player.inner.bot_mut().unwrap().action_buffer) {
                BotAction::Some(command) => {
                    if player.regulator.active() {
                        let _ = service.player_command(command, player_id, player);
                    }
                }
                BotAction::None(_) => {}
                BotAction::Quit => {
                    // Recycle.
                    if player.regulator.active() {
                        service.player_quit(player_id, player);
                    }
                    player.regulator.leave();
                    player.inner = PlayerInner::Bot(PlayerBotData::default());
                    if player.regulator.join() {
                        debug_assert!(false, "too early");
                        service.player_joined(player_id, player);
                    }
                }
            };
        }
    }

    /// Spawns/despawns bots based on number of (real) player clients.
    pub(crate) fn update_count(
        &mut self,
        service: &mut G,
        players: &mut PlayerRepo<G>,
        settings: &EngineArenaSettings,
    ) {
        let count = if let Some(bots) = settings.bots {
            bots
        } else {
            let count = if G::Bot::AUTO.bot_percent >= 0 {
                (G::Bot::AUTO.bot_percent as usize)
                    .saturating_mul(players.real_players_live as usize)
                    / 100
            } else {
                G::Bot::AUTO.max_bots.saturating_add_signed(
                    G::Bot::AUTO
                        .bot_percent
                        .saturating_mul(players.real_players_live as isize)
                        / 100,
                )
            };

            count.clamp(G::Bot::AUTO.min_bots, G::Bot::AUTO.max_bots) as u16
        };

        self.set_count(count as usize, service, players);
    }

    /// Changes number of bots by spawning/despawning.
    fn set_count(&mut self, count: usize, service: &mut G, players: &mut PlayerRepo<G>) {
        // Give server 3 seconds (50 ticks) to create all testing bots.
        let mut governor = 4.max(count / 50);

        while count < self.count && governor > 0 {
            self.count -= 1;
            governor -= 1;

            let player_id = PlayerId::nth_bot(self.count).unwrap();
            let player = players.get_mut(player_id).unwrap();
            if player.regulator.active() {
                service.player_quit(player_id, player);
            }
            player.regulator.leave();
        }

        while count > self.count && governor > 0 {
            governor -= 1;

            if let Some(next_id) = PlayerId::nth_bot(self.count) {
                let player = players
                    .entry(next_id)
                    .or_insert_with(|| Player::new(PlayerInner::Bot(PlayerBotData::default())));
                if player.regulator.join() {
                    service.player_joined(next_id, player);
                }
                self.count += 1;
            } else {
                debug_assert!(false, "should not run out of ids");
            }
        }
    }
}
