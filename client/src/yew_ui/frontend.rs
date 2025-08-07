// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::NexusRoute;
use crate::js_hooks::console_log;
use crate::{
    ArenaQuery, BrowserStorages, ChatRequest, ClientRequest, CommonSettings, GameClient,
    GameConstants, InvitationRequest, RankNumber, RegionId, ServerId, StrongCoreState, Translator,
    WeakCoreState,
};
use std::ops::Deref;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use web_sys::window;
use yew::{hook, use_context, Callback, Html, Properties};

#[derive(Properties, PartialEq)]
pub struct PropertiesWrapper<P: PartialEq> {
    pub props: P,
}

impl<P: PartialEq> Deref for PropertiesWrapper<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.props
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Features {
    pub cheats: bool,
    /// Show cookie consent dialogue.
    pub cookie_consent: bool,
    /// Show ad-provider settings menu button.
    pub ad_privacy: bool,
    pub chat: bool,
    pub outbound: OutboundLinks,
    pub account_exempt_banner_ads: bool,
    pub account_exempt_interstitial_ads: bool,
    pub account_exempt_rewarded_ads: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            cheats: false,
            cookie_consent: false,
            ad_privacy: false,
            chat: false,
            outbound: Default::default(),
            account_exempt_banner_ads: false,
            account_exempt_interstitial_ads: false,
            account_exempt_rewarded_ads: false,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum Escaping {
    /// In game, possibly pointer lock.
    InGame,
    /// Escape menu, possibly loss of pointer lock.
    Escaping {
        /// User didn't want to escape, but they still need to click to
        /// enter pointer lock.
        ///
        /// If not(cfg(feature = "pointer_lock")), should always be `false`.
        awaiting_pointer_lock: bool,
    },
    /// Spawn dialog open.
    #[default]
    Spawning,
}

impl Escaping {
    pub fn message(self) -> &'static str {
        match self {
            Self::InGame => "inGame",
            Self::Escaping { .. } => "escaping",
            Self::Spawning => "spawning",
        }
    }

    pub fn post_message(self) {
        post_message(self.message());
    }

    pub fn is_in_game(self) -> bool {
        matches!(self, Self::InGame)
    }

    pub fn is_escaping(self) -> bool {
        matches!(self, Self::Escaping { .. })
    }

    /// User didn't want to escape, but they still need to click to
    /// enter pointer lock.
    ///
    /// If not(cfg(feature = "pointer_lock")), should always be `false`.
    pub fn is_escaping_awaiting_pointer_lock(self) -> bool {
        matches!(
            self,
            Self::Escaping {
                awaiting_pointer_lock: true
            }
        )
    }

    pub fn is_spawning(self) -> bool {
        matches!(self, Self::Spawning)
    }

    pub fn toggle(self) -> Option<Self> {
        Some(match self {
            Self::InGame
            | Self::Escaping {
                awaiting_pointer_lock: true,
            } => Self::Escaping {
                awaiting_pointer_lock: false,
            },
            Self::Escaping {
                awaiting_pointer_lock: false,
            } => Self::InGame,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum InvitationLinks {
    None,
    #[default]
    Normal,
    /// A string containing `GAME_WILL_REPLACE` for adding the invitation code
    /// to produce an invitation link.
    Template(Rc<str>),
    Verbatim(Rc<str>),
}

impl InvitationLinks {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    pub fn is_template(&self) -> bool {
        matches!(self, Self::Template(_))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Accounts {
    None,
    /// e.g. Discord.
    Normal,
    /// e.g. Crazygames.
    Snippet {
        provider: String,
        sign_out: bool,
    },
}

impl Accounts {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    pub fn is_snippet(&self) -> bool {
        matches!(self, Self::Snippet { .. })
    }

    pub fn snippet(&self) -> Option<&str> {
        if let Self::Snippet { provider, .. } = self {
            Some(provider.as_str())
        } else {
            None
        }
    }

    /// Whether profile should support sign out.
    pub fn sign_out(&self) -> bool {
        if let Self::Snippet { sign_out, .. } = self {
            *sign_out
        } else {
            true
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutboundLinks {
    pub accounts: Accounts,
    pub app_stores: bool,
    pub credits: bool,
    pub contact_info: bool,
    pub invitations: InvitationLinks,
    pub social_media: bool,
    pub promo: bool,
}

impl OutboundLinks {
    pub const ALL: Self = Self {
        accounts: Accounts::Normal,
        app_stores: true,
        credits: true,
        contact_info: true,
        invitations: InvitationLinks::Normal,
        social_media: true,
        promo: true,
    };
    pub const NONE: Self = Self {
        accounts: Accounts::None,
        app_stores: false,
        credits: false,
        contact_info: false,
        invitations: InvitationLinks::None,
        social_media: false,
        promo: false,
    };

    pub fn any(&self) -> bool {
        self != &Self::NONE
    }
}

impl Default for OutboundLinks {
    fn default() -> Self {
        Self {
            accounts: Accounts::None,
            app_stores: false,
            credits: true,
            contact_info: true,
            social_media: false,
            invitations: InvitationLinks::default(),
            promo: true,
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum BannerAd {
    Unavailable,
    Available { request: Callback<()> },
}

#[derive(Clone, PartialEq)]
pub enum InterstitialAd {
    Unavailable,
    Available {
        /// Start watching (callback called on watch OR cancel).
        request: Callback<Option<Callback<()>>>,
    },
    Watching {
        /// Called when finished.
        callback: Option<Callback<()>>,
    },
}

impl InterstitialAd {
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

#[derive(Clone, PartialEq)]
pub enum RewardedAd {
    Unavailable,
    Available {
        /// Start watching (callback called on watch).
        request: Callback<Option<Callback<()>>>,
    },
    Watching {
        /// Called when finished.
        callback: Option<Callback<()>>,
    },
    /// Only used if `Watching.callback` is `None`.
    Watched {
        /// Set back to available.
        consume: Callback<()>,
    },
}

impl RewardedAd {
    pub fn available(&self) -> Option<&Callback<Option<Callback<()>>>> {
        if let Self::Available { request } = self {
            Some(request)
        } else {
            None
        }
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

/// Non-game-specific context wrapper.
#[allow(clippy::type_complexity)]
#[derive(Clone, PartialEq)]
pub struct Ctw {
    pub game_constants: &'static GameConstants,
    /// Show escape menu
    pub escaping: Escaping,
    pub nexus: bool,
    /// Copy of `context.mouse.pointer_locked`
    #[cfg(feature = "pointer_lock")]
    pub pointer_locked: bool,
    pub features: Features,
    pub banner_ad: BannerAd,
    pub interstitial_ad: InterstitialAd,
    pub rewarded_ad: RewardedAd,
    pub setting_cache: CommonSettings,
    pub change_common_settings_callback:
        Callback<Box<dyn FnOnce(&mut CommonSettings, &mut BrowserStorages)>>,
    pub set_server_id_callback: Callback<(ServerId, ArenaQuery)>,
    pub chat_request_callback: Callback<ChatRequest>,
    pub client_request_callback: Callback<ClientRequest>,
    pub invitation_request_callback: Callback<InvitationRequest>,
    pub raw_zoom_callback: Callback<f32>,
    pub recreate_renderer_callback: Callback<()>,
    pub set_context_menu_callback: Callback<Option<Html>>,
    pub set_escaping_callback: Callback<Escaping>,
    pub routes: Rc<Vec<NexusRoute>>,
    /// A copy of the core state.
    pub state: WeakCoreState,
    pub licenses: &'static str,
    pub translator: Translator,
    pub available_servers: Box<[ServerId]>,
    pub translate_rank_number: fn(&Translator, RankNumber) -> String,
    pub translate_rank_benefits: fn(&Translator, RankNumber) -> Vec<String>,
}

impl Ctw {
    pub fn current_region_id(&self) -> Option<RegionId> {
        let state = self.state.as_strong();
        self.setting_cache.server_id.and_then(|server_id| {
            state
                .servers
                .iter()
                .find(|((sid, _), _)| *sid == server_id)
                .map(|(_, s)| s.region_id)
        })
    }
}

#[hook]
pub fn use_banner_ad() -> BannerAd {
    use_ctw().banner_ad
}

#[hook]
pub fn use_interstitial_ad() -> InterstitialAd {
    use_ctw().interstitial_ad
}

#[hook]
pub fn use_rewarded_ad() -> RewardedAd {
    use_ctw().rewarded_ad
}

#[hook]
pub fn use_chat_request_callback() -> Callback<ChatRequest> {
    use_ctw().chat_request_callback
}

#[hook]
pub fn use_client_request_callback() -> Callback<ClientRequest> {
    use_ctw().client_request_callback
}

#[hook]
pub fn use_invitation_request_callback() -> Callback<InvitationRequest> {
    use_ctw().invitation_request_callback
}

#[allow(clippy::type_complexity)]
#[hook]
pub fn use_change_common_settings_callback(
) -> Callback<Box<dyn FnOnce(&mut CommonSettings, &mut BrowserStorages)>> {
    use_ctw().change_common_settings_callback
}

#[hook]
pub fn use_set_context_menu_callback() -> Callback<Option<Html>> {
    use_ctw().set_context_menu_callback
}

#[hook]
pub fn use_core_state() -> StrongCoreState<'static> {
    use_ctw().state.into_strong()
}

#[hook]
pub fn use_ctw() -> Ctw {
    use_context::<Ctw>().unwrap()
}

#[hook]
pub fn use_game_constants() -> &'static GameConstants {
    use_ctw().game_constants
}

#[hook]
pub fn use_raw_zoom_callback() -> Callback<f32> {
    use_ctw().raw_zoom_callback
}

#[hook]
pub fn use_features() -> Features {
    use_ctw().features
}

/// Game-specific context wrapper.
#[allow(clippy::type_complexity)]
pub struct Gctw<G: GameClient> {
    pub send_ui_event_callback: Callback<G::UiEvent>,
    pub change_settings_callback:
        Callback<Box<dyn FnOnce(&mut G::GameSettings, &mut BrowserStorages)>>,
    pub settings_cache: G::GameSettings,
}

impl<G: GameClient> Clone for Gctw<G> {
    fn clone(&self) -> Self {
        Self {
            send_ui_event_callback: self.send_ui_event_callback.clone(),
            change_settings_callback: self.change_settings_callback.clone(),
            settings_cache: self.settings_cache.clone(),
        }
    }
}

impl<G: GameClient> PartialEq for Gctw<G> {
    fn eq(&self, other: &Self) -> bool {
        self.send_ui_event_callback
            .eq(&other.send_ui_event_callback)
            && self
                .change_settings_callback
                .eq(&other.change_settings_callback)
            && self.settings_cache == other.settings_cache
    }
}

/// Only works in function component.
#[hook]
pub fn use_ui_event_callback<G: GameClient>() -> Callback<G::UiEvent> {
    use_gctw::<G>().send_ui_event_callback
}

#[allow(clippy::type_complexity)]
#[hook]
pub fn use_change_settings_callback<G: GameClient>(
) -> Callback<Box<dyn FnOnce(&mut G::GameSettings, &mut BrowserStorages)>> {
    use_gctw::<G>().change_settings_callback
}

#[hook]
pub fn use_gctw<G: GameClient>() -> Gctw<G> {
    use_context::<Gctw<G>>().unwrap()
}

/// Post message to window.
pub fn post_message(message: &str) {
    if window()
        .unwrap()
        .post_message(&JsValue::from_str(message), "*")
        .is_err()
    {
        console_log!("error posting message");
    }
}
