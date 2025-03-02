// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{
    entry_point, Apply, ClientContext, KeyboardEvent, LocalSettings, MouseEvent, PropertiesWrapper,
    RoutableExt, TranslateFn, Translator, VisibilityEvent,
};
use kodiak_common::bitcode::*;
use kodiak_common::{
    ClientUpdate, FatalError, GameConstants, NoGameArenaSettings, RankNumber, SceneId,
};
use serde::Serialize;
use yew::BaseComponent;

/// A modular game client-side.
pub trait GameClient: Sized + 'static {
    /// Audio files to play.
    #[cfg(feature = "audio")]
    type Audio: crate::io::Audio;
    /// Game-specific command to server.
    type GameRequest: 'static + Encode;
    /// Game-specific state.
    type GameState: Apply<Self::GameUpdate>;
    /// Event from game UI.
    type UiEvent;
    /// Properties sent to game UI.
    type UiProps: Default + PartialEq + Clone + 'static;
    /// Yew component of UI.
    type Ui: BaseComponent<Properties = PropertiesWrapper<Self::UiProps>>;
    /// Game-specific Nexus pages.
    type UiRoute: RoutableExt;
    /// Game-specific update from server.
    type GameUpdate: 'static + DecodeOwned;
    /// Game-specific settings
    type GameSettings: LocalSettings + Clone + PartialEq + Default;
    type ArenaSettings: Clone + Default + PartialEq + Default + Serialize = NoGameArenaSettings;
    /// Open source licenses.
    const LICENSES: &'static str = "";

    const GAME_CONSTANTS: &'static GameConstants;
    /// If game doesn't use tab, dedicate it to escape feature. Benefit is
    /// that pointer lock exited this way can be rerequested instantly.
    const TAB_TO_ESCAPE: bool = false;
    /// Like `TAB_TO_ESCAPE` but for 'p'.
    const P_TO_ESCAPE: bool = true;

    fn new(context: &mut ClientContext<Self>) -> Result<Self, FatalError>;

    fn describe_scene_id(scene_id: SceneId) -> Option<TranslateFn> {
        let _ = scene_id;
        None
    }

    fn translate_rank_number(_t: &Translator, n: RankNumber) -> String {
        /*
        match n {
            RankNumber::Rank1 => translate!(t, ""),
            RankNumber::Rank2 => translate!(t, ""),
            RankNumber::Rank3 => translate!(t, ""),
            RankNumber::Rank4 => translate!(t, ""),
            RankNumber::Rank5 => translate!(t, ""),
            RankNumber::Rank6 => translate!(t, ""),
        }

        Mk48/Netquel:
        - Ensign
        - Lieutenant
        - Commander
        - Captain
        - Colonel
        - Admiral

        Zentakil:
        - Plankton
        - Protozoa
        - Amoeba
        - Kraken
        - Elder
        - Guardian
         */

        n.to_string()
    }

    fn translate_rank_benefits(_t: &Translator, _n: RankNumber) -> Vec<String> {
        /*
        match n {
            RankNumber::Rank1 => vec![],
            RankNumber::Rank2 => vec![],
            RankNumber::Rank3 => vec![],
            RankNumber::Rank4 => vec![],
            RankNumber::Rank5 => vec![],
            RankNumber::Rank6 => vec![],
        }
        */
        Vec::new()
    }

    /// Peek at a core update before it is applied to `CoreState`.
    fn peek_core(&mut self, _inbound: &ClientUpdate, _context: &mut ClientContext<Self>) {}

    /// Peek at a game update before it is applied to `GameState`.
    fn peek_game(&mut self, inbound: &Self::GameUpdate, _context: &mut ClientContext<Self>) {
        let _ = inbound;
    }

    /// Peek at a keyboard event before it is applied to `KeyboardState`.
    fn peek_keyboard(&mut self, _event: &KeyboardEvent, _context: &mut ClientContext<Self>) {}

    /// Peek at a mouse event before it is applied to `MouseState`.
    fn peek_mouse(&mut self, event: &MouseEvent, _context: &mut ClientContext<Self>) {
        let _ = event;
    }

    /// Peek at a visibility event before it is applied to `VisibilityState`.
    fn peek_visibility(&mut self, event: &VisibilityEvent, _context: &mut ClientContext<Self>) {
        let _ = event;
    }

    /// Render the game. Optional, as this may be done in `tick`. Must end with a call to
    /// [`Renderer::render`].
    fn render(&mut self, _elapsed_seconds: f32, _context: &ClientContext<Self>) {}
    /// A game with update and render intertwined implements this method.
    /// Otherwise, it implements update() and render().
    fn tick(&mut self, elapsed_seconds: f32, context: &mut ClientContext<Self>) {
        self.update(elapsed_seconds, context);
        self.render(elapsed_seconds, context);
    }

    /// Peek at a UI event before it is applied to `UiState`.
    fn ui(&mut self, event: Self::UiEvent, _context: &mut ClientContext<Self>) {
        let _ = event;
    }

    /// Updates the game. Optional, as may be done in `tick`.
    fn update(&mut self, _elapsed_seconds: f32, _context: &mut ClientContext<Self>) {}

    /// Run the game.
    fn run() {
        entry_point::<Self>();
    }
}
