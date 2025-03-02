// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::shard_context::ShardContextProvider;
use super::{BotOptions, ShardPerRealm};
use crate::bitcode::*;
use crate::service::{ArenaContext, Player, Score};
use crate::{
    ArenaId, ArenaSettingsDto, GameConstants, NoGameArenaSettings, PlayerAlias, PlayerId, ServerId,
    TeamId, TeamName,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::time::Duration;

/// A modular game service (representing one arena).
pub trait ArenaService: 'static + Unpin + Sized + Send + Sync {
    /// The length of a tick in seconds.
    const TICK_PERIOD_SECS: f32;
    /// How long a player can remain in limbo after they lose connection.
    const LIMBO: Duration = Duration::from_secs(6);
    /// How many players to display on the leaderboard (and liveboard).
    const LEADERBOARD_SIZE: usize = 10;
    /// Whether to display bots on liveboard. Bots are never saved to the leaderboard.
    const LIVEBOARD_BOTS: bool = cfg!(debug_assertions);
    /// Leaderboard won't be touched if player count is below.
    const LEADERBOARD_MIN_PLAYERS: u32 = 10;
    /// Display team name in place of player alias on liveboard.
    ///
    /// Useful when score is stored in the team to avoid leaderboard exploitation.
    const LIVEBOARD_LEADERBOARD_TEAM_REPRESENTATION: bool = false;
    const GAME_CONSTANTS: &'static GameConstants;
    const MAX_TEMPORARY_SERVERS: usize = 16;

    type Bot: 'static + Bot<Self> + Debug = ();
    type ClientData: 'static + Default + Debug + Unpin + Send + Sync = ();
    type GameUpdate: 'static + Sync + Send + Encode + DecodeOwned;
    type GameRequest: 'static + Debug + DecodeOwned + Send + Unpin;
    type Shard: ShardContextProvider<Self> = ShardPerRealm;
    type ArenaSettings: 'static
        + Sync
        + Send
        + Unpin
        + Debug
        + Clone
        + Default
        + PartialEq
        + Serialize
        + DeserializeOwned = NoGameArenaSettings;

    /// Creates a service with the default `Tier` if applicable.
    fn new(context: &mut ArenaContext<Self>) -> Self;

    /// Get alias of authority figure (that, for example, sends chat moderation warnings).
    fn authority_alias() -> PlayerAlias {
        PlayerAlias::new_unsanitized("Server")
    }

    /// Returns true iff the player is considered to be "alive":
    /// - on leaderboard
    /// - counts as a "play"
    fn is_alive(&self, player_id: PlayerId) -> bool;

    fn get_score(&self, player_id: PlayerId) -> Score;
    fn get_alias(&self, player_id: PlayerId) -> PlayerAlias;

    /// Moderator override alias.
    fn override_alias(&mut self, player_id: PlayerId, alias: PlayerAlias) {
        let _ = (player_id, alias);
    }

    fn get_team_id(&self, player_id: PlayerId) -> Option<TeamId> {
        let _ = player_id;
        None
    }

    fn get_team_name(&self, player_id: PlayerId) -> Option<TeamName> {
        let _ = player_id;
        None
    }

    fn get_team_members(&self, player_id: PlayerId) -> Option<Vec<PlayerId>> {
        let _ = player_id;
        None
    }

    /// Return iff the player is forced to whisper chat.
    fn force_whisper(&self, player_id: PlayerId) -> bool {
        let _ = player_id;
        false
    }

    /// Game should add the player.
    fn player_joined(&mut self, player_id: PlayerId, _player: &mut Player<Self>) {
        let _ = player_id;
    }

    /// Game should idempotently kill the player.
    fn player_quit(&mut self, player_id: PlayerId, _player: &mut Player<Self>) {
        let _ = player_id;
    }

    /// Game should forget the player. Can be called as early as 1 tick after `player_quit`.
    fn player_left(&mut self, player_id: PlayerId, _player: &mut Player<Self>) {
        let _ = player_id;
    }

    /// Game should process player command.
    fn player_command(
        &mut self,
        request: Self::GameRequest,
        player_id: PlayerId,
        _player: &mut Player<Self>,
    ) -> Option<Self::GameUpdate>;

    /// Game should interpret player chat command.
    fn chat_command(
        &mut self,
        command: &str,
        player_id: PlayerId,
        player: &mut Player<Self>,
    ) -> Option<String> {
        let _ = (command, player_id, player);
        None
    }

    fn server_message(
        &mut self,
        server_id: ServerId,
        arena_id: ArenaId,
        message: serde_json::Value,
        context: &mut ArenaContext<Self>,
    ) {
        // No-op
        let _ = (server_id, arena_id, message, context);
    }

    /// Gets a client a.k.a. real player's [`GameUpdate`].
    ///
    /// May return one update to be send reliably and/or use
    /// `player.client().unwrap().send_with_reliable()` for
    /// more flexibility.
    fn get_game_update(
        &self,
        player_id: PlayerId,
        player: &mut Player<Self>,
    ) -> Option<Self::GameUpdate>;

    /// Before sending.
    fn tick(&mut self, context: &mut ArenaContext<Self>);
    /// After sending.
    fn post_update(&mut self, context: &mut ArenaContext<Self>) {
        let _ = context;
    }

    /// For metrics.
    fn entities(&self) -> usize;
    /// For metrics.
    fn world_size(&self) -> f32;
}

/// Implemented by game bots.
pub trait Bot<G: ArenaService>: Default + Unpin + Sized + Send {
    const AUTO: BotOptions = BotOptions {
        min_bots: 30,
        max_bots: 128,
        bot_percent: 80,
    };

    /// `Quit` indicates quitting.
    fn update(
        game: &G,
        player_id: PlayerId,
        _player: &mut Player<G>,
        settings: &ArenaSettingsDto<G::ArenaSettings>,
    ) -> BotAction<G::GameRequest>;
}

#[derive(Debug)]
pub enum BotAction<GR> {
    Some(GR),
    None(&'static str),
    Quit,
}

impl<GR> Default for BotAction<GR> {
    fn default() -> Self {
        Self::None("default")
    }
}

// Useful as a placeholder.
impl<G: ArenaService> Bot<G> for () {
    const AUTO: BotOptions = BotOptions {
        min_bots: 0,
        max_bots: 0,
        bot_percent: 0,
    };

    fn update(
        _game: &G,
        _player_id: PlayerId,
        _player: &mut Player<G>,
        _settings: &ArenaSettingsDto<G::ArenaSettings>,
    ) -> BotAction<G::GameRequest> {
        BotAction::default()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::ArenaService;
    use crate::{
        ArenaContext, Bot, BotAction, GameId, Player, PlayerAlias, PlayerId, Score, TierNumber,
    };

    pub struct MockGame;

    #[derive(Debug, Default)]
    pub struct MockGameBot;

    impl Bot<MockGame> for MockGameBot {
        fn update(_: &MockGame, _: PlayerId, _: &mut Player<MockGame>) -> BotAction<()> {
            Default::default()
        }
    }

    impl ArenaService for MockGame {
        type Bot = MockGameBot;
        type ClientData = ();
        type GameRequest = ();
        type GameUpdate = ();

        const GAME_ID: GameId = GameId::Redacted;
        const TICK_PERIOD_SECS: f32 = 0.5;

        fn new(_: Option<TierNumber>, _: usize) -> Self {
            Self
        }

        fn get_score(&self, _: PlayerId) -> Score {
            Default::default()
        }

        fn get_alias(&self, _: PlayerId) -> PlayerAlias {
            Default::default()
        }

        fn player_command(
            &mut self,
            _: Self::GameRequest,
            _: PlayerId,
            _: &mut Player<Self>,
        ) -> Option<Self::GameUpdate> {
            None
        }

        fn get_game_update(&self, _: PlayerId, _: &mut Player<Self>) -> Option<Self::GameUpdate> {
            Some(())
        }

        fn is_alive(&self, _: PlayerId) -> bool {
            false
        }

        fn tick(&mut self, _: &mut ArenaContext<Self>) {}

        fn entities(&self) -> usize {
            Default::default()
        }

        fn world_size(&self) -> f32 {
            Default::default()
        }
    }
}
