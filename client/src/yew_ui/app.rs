// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::keyboard::KeyboardEventsListener;
use super::{
    logout, post_message, process_finish_signin, renew_session, Accounts, BannerAd, Canvas, Ctw,
    ErrorTracer, EscapeMenu, Escaping, FatalErrorDialog, Features, FeedbackDialog, Gctw,
    GlobalEventListener, InterstitialAd, InvitationLinks, LicensingDialog, OutboundLinks,
    PrivacyDialog, ProfileDialog, RanksDialog, Reconnecting, RewardedAd, SetLogin, SetLoginAlias,
    SettingsDialog, StoreDialog, TermsDialog,
};
use crate::js_hooks::console_log;
use crate::net::{SocketUpdate, SystemInfo};
use crate::{
    browser_pathname, eval_snippet, translate, AdEvent, ArenaQuery, BannerAdEvent, BrowserStorages,
    ChatRequest, ClientBroker, ClientContext, ClientRequest, CommonRequest, CommonSettings,
    CommonUpdate, FatalError, GameClient, InvitationId, InvitationRequest, LocalSettings,
    NexusPath, PlayWithFriendsDialog, PlayerAlias, QuestEvent, RealmId, RealmName, Referrer,
    ServerId, ServerKind, SmolRoutable, TranslationCache, Translations, Translator, VideoAdEvent,
    WeakCoreState,
};
use gloo_render::{request_animation_frame, AnimationFrame};
use std::collections::HashMap;
use std::num::NonZeroU8;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;
use stylist::{global_style, GlobalStyle};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::future_to_promise;
use web_sys::{FocusEvent, KeyboardEvent, MessageEvent, MouseEvent, TouchEvent, WheelEvent};
use yew::prelude::{html, Callback, Component, Context, ContextProvider, Event, Html, Properties};
use yew::AttrValue;
use yew_router::prelude::{BrowserRouter, Routable, Switch};

pub const CONTACT_EMAIL: &str = "contact@softbear.com";

struct App<G: GameClient> {
    context_menu: Option<Html>,
    client_broker: PendingBroker<G>,
    ui_event_buffer: Vec<G::UiEvent>,
    ui_props: G::UiProps,
    banner_ad: BannerAd,
    interstitial_ad: InterstitialAd,
    rewarded_ad: RewardedAd,
    /// After [`AppMsg::RecreateCanvas`] is received, before [`AppMsg::RecreateRenderer`] is received.
    recreating_canvas: RecreatingCanvas,
    features: Features,
    translation_cache: TranslationCache,
    _animation_frame: AnimationFrame,
    _keyboard_events_listener: KeyboardEventsListener,
    _visibility_listener: GlobalEventListener<Event>,
    #[cfg(feature = "pointer_lock")]
    _pointer_lock_listener: GlobalEventListener<Event>,
    /// Message from parent window.
    _message_listener: GlobalEventListener<MessageEvent>,
    _context_menu_inhibitor: GlobalEventListener<MouseEvent>,
    #[cfg(feature = "mouse_over_ui")]
    _mouse_over_ui: Vec<GlobalEventListener<MouseEvent>>,
    #[cfg(feature = "mouse_over_ui")]
    _touch_over_ui: Vec<GlobalEventListener<TouchEvent>>,
    #[cfg(feature = "mouse_over_ui")]
    _wheel_over_ui: Vec<GlobalEventListener<WheelEvent>>,
    _error_tracer: ErrorTracer,
    _global_style: GlobalStyle,
}

#[allow(clippy::large_enum_variant)]
enum PendingBroker<G: GameClient> {
    Done(ClientBroker<G>),
    /// Failed to create the game, but stay ready to contact the serve.
    Failed {
        context: ClientContext<G>,
        error: FatalError,
    },
    /// Contains things that the client_broker will eventually own, but that are required to exist
    /// before the client_broker.
    Pending {
        browser_storages: BrowserStorages,
        common_settings: CommonSettings,
        settings: G::GameSettings,
    },
    /// Used to help replace [`Pending`] with [`Done`] in lieu of
    /// https://github.com/rust-lang/rfcs/pull/1736
    Swapping,
}

impl<G: GameClient> PendingBroker<G> {
    fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    #[allow(unused)]
    fn as_ref(&self) -> Option<&ClientBroker<G>> {
        match self {
            Self::Done(client_broker) => Some(client_broker),
            Self::Failed { .. } => None,
            Self::Pending { .. } => None,
            Self::Swapping => {
                debug_assert!(false, "PendingBroker::Swapping::as_ref");
                None
            }
        }
    }

    fn as_mut(&mut self) -> Option<&mut ClientBroker<G>> {
        match self {
            Self::Done(client_broker) => Some(client_broker),
            Self::Failed { .. } => None,
            Self::Pending { .. } => None,
            Self::Swapping => {
                debug_assert!(false, "PendingBroker::Swapping::as_mut");
                None
            }
        }
    }

    fn as_context_ref(&self) -> Option<&ClientContext<G>> {
        match self {
            Self::Done(client_broker) => Some(&client_broker.context),
            Self::Failed { context, .. } => Some(context),
            Self::Pending { .. } => None,
            Self::Swapping => {
                debug_assert!(false, "PendingBroker::Swapping::as_context_ref");
                None
            }
        }
    }

    fn as_context_mut(&mut self) -> Option<&mut ClientContext<G>> {
        match self {
            Self::Done(client_broker) => Some(&mut client_broker.context),
            Self::Failed { context, .. } => Some(context),
            Self::Pending { .. } => None,
            Self::Swapping => {
                debug_assert!(false, "PendingBroker::Swapping::as_context_mut");
                None
            }
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq)]
enum RecreatingCanvas {
    /// No canvas recreation is in progress.
    #[default]
    None,
    /// Canvas is removed.
    Started,
    /// Canvas is restored.
    Finished,
}

#[derive(Default, PartialEq, Properties)]
struct AppProps {}

/// Enum of the engine client messages App can handle.
#[allow(clippy::type_complexity)]
enum AppMsg<G: GameClient> {
    ChangeCommonSettings(Box<dyn FnOnce(&mut CommonSettings, &mut BrowserStorages)>),
    ChangeSettings(Box<dyn FnOnce(&mut G::GameSettings, &mut BrowserStorages)>),
    FrontendCreated(Option<SystemInfo>),
    Login(SetLogin),
    /// Signals the canvas should be recreated, followed by the renderer.
    RecreateCanvas,
    /// Put back the canvas.
    #[doc(hidden)]
    RecreateCanvasPart2,
    /// Signals just the renderer should be recreated.
    RecreateRenderer,
    SetServerId(ServerId, ArenaQuery),
    Frame {
        time: f64,
    },
    Socket(SocketUpdate<CommonUpdate<G::GameUpdate>>),
    KeyboardFocus(FocusEvent),
    Keyboard(KeyboardEvent),
    MouseFocus(FocusEvent),
    Mouse(MouseEvent),
    RawZoom(f32),
    SendChatRequest(ChatRequest),
    SendClientRequest(ClientRequest),
    SendInvitationRequest(InvitationRequest),
    SendUiEvent(G::UiEvent),
    SetContextMenuProps(Option<Html>),
    SetUiProps(G::UiProps),
    SetEscaping(Escaping),
    RecordNexusPath(Option<NexusPath>),
    Touch(TouchEvent),
    /// Error trace.
    Trace(String),
    VisibilityChange(Event),
    #[cfg(feature = "pointer_lock")]
    PointerLockChange,
    /// Show a banner ad.
    RequestBannerAd,
    /// Play an interstital ad.
    RequestInterstitialAd(Option<Callback<()>>),
    /// Play a rewarded ad.
    RequestRewardedAd(Option<Callback<()>>),
    /// Make another rewarded ad available.
    ConsumeRewardedAd,
    /// Message from parent window.
    Message(JsValue),
    Wheel(WheelEvent),
}

impl<G: GameClient> App<G> {
    pub fn create_animation_frame(ctx: &Context<Self>) -> AnimationFrame {
        let link = ctx.link().clone();
        request_animation_frame(move |time| link.send_message(AppMsg::Frame { time }))
    }
}

impl<G: GameClient> Component for App<G> {
    type Message = AppMsg<G>;
    type Properties = AppProps;

    fn create(ctx: &Context<Self>) -> Self {
        let keyboard_callback = ctx.link().callback(AppMsg::Keyboard);
        let keyboard_focus_callback = ctx.link().callback(AppMsg::KeyboardFocus);
        let visibility_callback = ctx.link().callback(AppMsg::VisibilityChange);
        #[cfg(feature = "pointer_lock")]
        let pointer_lock_callback = ctx.link().callback(|_| AppMsg::PointerLockChange);
        let message_callback = ctx.link().callback(AppMsg::Message);
        let trace_callback = ctx.link().callback(AppMsg::Trace);

        // First load local storage common settings.
        // Not guaranteed to set either or both to Some. Could fail to load.
        let browser_storages = BrowserStorages::default();
        let common_settings = CommonSettings::load(&browser_storages, CommonSettings::default());
        let settings = G::GameSettings::load(&browser_storages, G::GameSettings::default());

        renew_session(
            ctx.link().callback(|login| {
                AppMsg::Login(SetLogin {
                    login,
                    alias: SetLoginAlias::NoEffect,
                    quit: false,
                })
            }),
            common_settings.session_id,
            G::GAME_CONSTANTS.game_id(),
        );

        #[cfg(feature = "mouse_over_ui")]
        let mut _mouse_over_ui = Vec::new();
        #[cfg(feature = "mouse_over_ui")]
        let mut _touch_over_ui = Vec::new();
        #[cfg(feature = "mouse_over_ui")]
        let mut _wheel_over_ui = Vec::new();
        #[cfg(feature = "mouse_over_ui")]
        {
            let callback = ctx.link().callback(AppMsg::Mouse);
            _mouse_over_ui.push(GlobalEventListener::new_window(
                "mousedown",
                move |event: &MouseEvent| {
                    callback.emit(event.clone());
                },
                true,
            ));
            let callback = ctx.link().callback(AppMsg::Mouse);
            _mouse_over_ui.push(GlobalEventListener::new_window(
                "mouseup",
                move |event: &MouseEvent| {
                    callback.emit(event.clone());
                },
                true,
            ));
            let callback = ctx.link().callback(AppMsg::Mouse);
            _mouse_over_ui.push(GlobalEventListener::new_window(
                "mousemove",
                move |event: &MouseEvent| {
                    callback.emit(event.clone());
                },
                true,
            ));
            let callback = ctx.link().callback(AppMsg::Mouse);
            _mouse_over_ui.push(GlobalEventListener::new_document(
                "mouseenter",
                move |event: &MouseEvent| {
                    callback.emit(event.clone());
                },
                true,
            ));
            let callback = ctx.link().callback(AppMsg::Mouse);
            _mouse_over_ui.push(GlobalEventListener::new_document(
                "mouseleave",
                move |event: &MouseEvent| {
                    callback.emit(event.clone());
                },
                true,
            ));
            /*
            let callback = ctx.link().callback(AppMsg::Touch);
            _touch_over_ui.push(WindowEventListener::new("touchstart", move |event: &TouchEvent| {
                callback.emit(event.clone());
            }, true));
            let callback = ctx.link().callback(AppMsg::Touch);
            _touch_over_ui.push(WindowEventListener::new("touchend", move |event: &TouchEvent| {
                callback.emit(event.clone());
            }, true));
            let callback = ctx.link().callback(AppMsg::Touch);
            _touch_over_ui.push(WindowEventListener::new("touchmove", move |event: &TouchEvent| {
                callback.emit(event.clone());
            }, true));
            */
            let callback = ctx.link().callback(AppMsg::Wheel);
            _wheel_over_ui.push(GlobalEventListener::new_window(
                "wheel",
                move |event: &WheelEvent| {
                    callback.emit(event.clone());
                },
                true,
            ));
        }

        let mut client_broker = PendingBroker::Pending {
            browser_storages,
            common_settings,
            settings,
        };
        let translation_cache = TranslationCache::default();

        // On first render, spawn a JS promise that will queue AppMsg::FrontendCreated.
        let frontend_created_callback = ctx.link().callback(AppMsg::FrontendCreated);
        let (settings, _, browser_storages) = settings_mut(&mut client_broker).unwrap();

        // Persist some things.
        settings.set_cohort_id(settings.cohort_id, browser_storages);
        settings.set_random_guest_alias(settings.random_guest_alias, browser_storages);

        let translation_entry = translation_cache.prepare_insert(settings.language).unwrap();
        let future = SystemInfo::new(
            settings.server_id,
            settings.arena_id,
            settings.cohort_id,
            settings.language,
            G::GAME_CONSTANTS.domain,
        );

        let _ = future_to_promise(async move {
            let system_info = future.await;
            if let Some(system_info) = &system_info {
                *translation_entry.borrow_mut() = Rc::new(Translations {
                    translations: system_info.translation.translations.deref().clone(),
                })
            }
            frontend_created_callback.emit(system_info);
            Ok(JsValue::NULL)
        });

        Self {
            context_menu: None,
            client_broker,
            ui_event_buffer: Vec::new(),
            ui_props: G::UiProps::default(),
            recreating_canvas: RecreatingCanvas::default(),
            banner_ad: BannerAd::Unavailable,
            interstitial_ad: InterstitialAd::Unavailable,
            rewarded_ad: RewardedAd::Unavailable,
            features: Features::default(),
            translation_cache,
            _animation_frame: Self::create_animation_frame(ctx),
            _keyboard_events_listener: KeyboardEventsListener::new(
                keyboard_callback,
                keyboard_focus_callback,
            ),
            _visibility_listener: GlobalEventListener::new_window(
                "visibilitychange",
                move |event: &Event| {
                    visibility_callback.emit(event.clone());
                },
                false,
            ),
            #[cfg(feature = "pointer_lock")]
            _pointer_lock_listener: GlobalEventListener::new_document(
                "pointerlockchange",
                move |_: &Event| {
                    pointer_lock_callback.emit(());
                },
                false,
            ),
            _message_listener: GlobalEventListener::new_window(
                "message",
                move |event: &MessageEvent| {
                    let data = event.data();
                    if data.is_string() || data.is_object() {
                        // event.clone() causes an infinite loop in Firefox
                        message_callback.emit(data);
                    }
                },
                false,
            ),
            _context_menu_inhibitor: GlobalEventListener::new_body(
                "contextmenu",
                move |event: &MouseEvent| event.prevent_default(),
                true,
            ),
            #[cfg(feature = "mouse_over_ui")]
            _mouse_over_ui,
            #[cfg(feature = "mouse_over_ui")]
            _touch_over_ui,
            #[cfg(feature = "mouse_over_ui")]
            _wheel_over_ui,
            _error_tracer: ErrorTracer::new(trace_callback),
            _global_style: global_style!(
                r#"
                html {
                    font-family: sans-serif;
                    font-size: 1.5vmin;
                    font-size: calc(7px + 0.8vmin);
                }

                body {
                    color: white;
                    margin: 0;
                    overflow: hidden;
                    padding: 0;
                    touch-action: none;
                    user-select: none;
                    user-drag: none;
                    -webkit-user-drag: none;
                }

                a {
                    color: white;
                }

                ul, ol {
                    padding-inline-start: 2rem;
                }

                img {
                    user-drag: none;
                    -webkit-user-drag: none;
                }
            "#
            )
            .expect("failed to mount global style"),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: AppMsg<G>) -> bool {
        let mut ret = false;

        match msg {
            AppMsg::ChangeCommonSettings(change) => {
                change_common_settings(
                    self.features.cookie_consent,
                    &mut self.client_broker,
                    change,
                );
                // We con't know if the settings affect the UI, so conservatively assume they do.
                ret = true;
            }
            AppMsg::ChangeSettings(change) => {
                change_settings(&mut self.client_broker, change);
                // We don't know if the settings affect the UI, so conservatively assume they do.
                ret = true;
            }
            AppMsg::FrontendCreated(system_info) => {
                assert!(self.client_broker.is_pending());
                if let Some(system_info) = &system_info {
                    for snippet in system_info.response.snippets.iter() {
                        eval_snippet(snippet);
                    }
                    self.translation_cache.languages = system_info.languages.clone().into();
                }
                self.client_broker =
                    match std::mem::replace(&mut self.client_broker, PendingBroker::Swapping) {
                        PendingBroker::Pending {
                            browser_storages,
                            common_settings,
                            settings,
                        } => {
                            match ClientBroker::new(
                                browser_storages,
                                common_settings,
                                settings,
                                ctx.link().callback(AppMsg::Socket),
                                ctx.link().callback(AppMsg::SetUiProps),
                                ctx.link().callback(AppMsg::SendUiEvent),
                                system_info,
                            ) {
                                Ok(mut client_broker) => PendingBroker::Done({
                                    for event in self.ui_event_buffer.drain(..) {
                                        client_broker.ui_event(event);
                                    }
                                    if !self.rewarded_ad.is_unavailable() {
                                        client_broker.context.enable_rewarded_ads();
                                    }
                                    client_broker
                                }),
                                Err((context, error)) => {
                                    // We tried :(
                                    PendingBroker::Failed { context, error }
                                }
                            }
                        }
                        PendingBroker::Swapping => {
                            unreachable!("client_broker creation aborted")
                        }
                        PendingBroker::Done(_) => {
                            unreachable!("client_broker already created")
                        }
                        PendingBroker::Failed { .. } => {
                            unreachable!("client_broker already failed")
                        }
                    }
            }
            AppMsg::Login(SetLogin {
                login,
                alias: set_alias,
                quit,
            }) => {
                if let Some(context) = self.client_broker.as_context_mut() {
                    context
                        .common_settings
                        .set_session_id(Some(login.session_id), &mut context.browser_storages);
                    context.common_settings.set_session_token(
                        Some(login.session_token),
                        &mut context.browser_storages,
                    );

                    context
                        .socket
                        .reset_host(ClientContext::<G>::compute_websocket_host(
                            &context.common_settings,
                            &context.system_info,
                            context.referrer,
                        ));
                    context.send_to_server(CommonRequest::Client(ClientRequest::Login(
                        login.session_token,
                    )));
                    if quit {
                        context.send_to_server(CommonRequest::Client(ClientRequest::Quit));
                    }

                    fn compute_alias(
                        user_name: Option<&String>,
                        nick_name: Option<&String>,
                    ) -> Option<PlayerAlias> {
                        nick_name
                            .map(|n| n.as_str())
                            .or(user_name.map(|u| {
                                if u.len() > PlayerAlias::capacity() {
                                    // Truncate less important parts. Could help CrazyGames random names and emails:
                                    // - HappyCat.6nOb -> HappyCat
                                    // - RoboticBroccoli.UTF4 -> RoboticBrocc
                                    // - finntbear@gmail.com -> finntbear
                                    let delimiters: &[char] = if u.contains('@') {
                                        &['@']
                                    } else {
                                        &['.', '_', '-', '@']
                                    };
                                    u.rsplit_once(delimiters)
                                        .map(|(b, _)| {
                                            &b[0..b.floor_char_boundary(PlayerAlias::capacity())]
                                        })
                                        .unwrap_or(u.as_str())
                                } else {
                                    u.as_str()
                                }
                            }))
                            .map(|n| (PlayerAlias::new_unsanitized(n), n))
                            .filter(|(p, n)| p.as_str() == *n)
                            .map(|(a, _)| a)
                    }
                    if matches!(set_alias, SetLoginAlias::NoEffect)
                        || (context.common_settings.alias.is_some()
                            && matches!(set_alias, SetLoginAlias::OverwriteGuestName))
                    {
                        // No-op.
                    } else if let Some(alias) =
                        compute_alias(login.user_name.as_ref(), login.nick_name.as_ref())
                    {
                        // Default alias when logging in.
                        context
                            .common_settings
                            .set_alias(Some(alias), &mut context.browser_storages);
                    } else if context.common_settings.alias.is_some()
                        && compute_alias(
                            context.common_settings.user_name.as_ref(),
                            context.common_settings.nick_name.as_ref(),
                        ) == context.common_settings.alias
                    {
                        // Logging out.
                        context
                            .common_settings
                            .set_alias(None, &mut context.browser_storages);
                    }
                    context
                        .common_settings
                        .set_user(login.user, &mut context.browser_storages);
                    context
                        .common_settings
                        .set_nick_name(login.nick_name, &mut context.browser_storages);
                    context
                        .common_settings
                        .set_user_name(login.user_name, &mut context.browser_storages);
                    context
                        .common_settings
                        .set_store_enabled(login.store_enabled, &mut context.browser_storages);

                    use crate::LocalSettings;
                    context
                        .common_settings
                        .synchronize(&login.settings, &mut context.browser_storages);
                    context
                        .settings
                        .synchronize(&login.settings, &mut context.browser_storages);
                }
            }
            AppMsg::RecreateCanvas => {
                self.recreating_canvas = RecreatingCanvas::Started;
                console_log!("started recreating canvas");
                ret = true;
            }
            AppMsg::RecreateCanvasPart2 => {
                self.recreating_canvas = RecreatingCanvas::Finished;
                console_log!("finished recreating canvas");
                ret = true;
            }
            AppMsg::RecreateRenderer => {
                self.recreating_canvas = RecreatingCanvas::None;
                console_log!("could not recreate renderer.");
                /*
                if let Some(client_broker) = self.client_broker.as_mut() {
                    if let Err(e) = client_broker.recreate_renderer() {
                        console_log!("could not recreate renderer: {}", e);
                    } else {
                        console_log!("finished recreating renderer");
                    }
                }
                */
                ret = true;
            }
            AppMsg::SetServerId(server_id, arena_id) => {
                if let Some(context) = self.client_broker.as_context_mut() {
                    context.send_to_server(CommonRequest::Client(ClientRequest::SwitchArena {
                        server_id,
                        arena_id,
                    }));
                }
            }
            AppMsg::Frame { time } => {
                if self.recreating_canvas != RecreatingCanvas::Started {
                    if let Some(client_broker) = self.client_broker.as_mut() {
                        client_broker.frame((time * 0.001) as f32);
                    }
                }
                self._animation_frame = Self::create_animation_frame(ctx);
            }
            AppMsg::Socket(update) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.socket_update(update);
                } else {
                    debug_assert!(false);
                }
            }
            AppMsg::Keyboard(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.keyboard(event);
                }
            }
            AppMsg::KeyboardFocus(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.keyboard_focus(event);
                }
            }
            AppMsg::Mouse(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.mouse(event);
                }
            }
            AppMsg::MouseFocus(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.mouse_focus(event);
                }
            }
            AppMsg::RawZoom(amount) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.raw_zoom(amount);
                }
            }
            AppMsg::SendChatRequest(request) => {
                if let Some(context) = self.client_broker.as_context_mut() {
                    context.send_request(CommonRequest::Chat(request));
                }
            }
            AppMsg::SendClientRequest(request) => {
                if let Some(context) = self.client_broker.as_context_mut() {
                    context.send_request(CommonRequest::Client(request));
                }
            }
            AppMsg::SendInvitationRequest(request) => {
                if let Some(context) = self.client_broker.as_context_mut() {
                    context.send_request(CommonRequest::Invitation(request));
                }
            }
            AppMsg::SetContextMenuProps(props) => {
                self.context_menu = props;
                ret = true;
            }
            AppMsg::SendUiEvent(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.ui_event(event);
                } else {
                    self.ui_event_buffer.push(event);
                }
            }
            AppMsg::SetUiProps(props) => {
                self.ui_props = props;
                ret = true;
            }
            AppMsg::SetEscaping(escaping) => {
                if let Some(context) = self.client_broker.as_context_mut() {
                    context.set_escaping(escaping);
                    ret = true;
                }
            }
            AppMsg::Touch(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.touch(event);
                }
            }
            AppMsg::RequestBannerAd => {
                if matches!(self.banner_ad, BannerAd::Available { .. }) {
                    if let Some(context) = self.client_broker.as_context_mut() {
                        context.ad_event(AdEvent::Banner(BannerAdEvent::Request));
                    }
                    post_message("requestBannerAd");
                }
            }
            AppMsg::RequestInterstitialAd(callback) => {
                if matches!(self.interstitial_ad, InterstitialAd::Available { .. }) {
                    if let Some(context) = self.client_broker.as_context_mut() {
                        context.ad_event(AdEvent::Interstitial(VideoAdEvent::Request));
                    }
                    self.interstitial_ad = InterstitialAd::Watching { callback };
                    post_message("requestInterstitialAd");
                }
            }
            AppMsg::RequestRewardedAd(callback) => {
                if matches!(self.rewarded_ad, RewardedAd::Available { .. }) {
                    if let Some(context) = self.client_broker.as_context_mut() {
                        context.ad_event(AdEvent::Rewarded(VideoAdEvent::Request));
                    }
                    self.rewarded_ad = RewardedAd::Watching { callback };
                    post_message("requestRewardedAd");
                }
            }
            AppMsg::ConsumeRewardedAd => {
                if matches!(self.rewarded_ad, RewardedAd::Watched { .. }) {
                    self.rewarded_ad = RewardedAd::Available {
                        request: ctx.link().callback(AppMsg::RequestRewardedAd),
                    };
                }
            }
            AppMsg::Trace(message) => {
                if let Some(context) = self.client_broker.as_context_mut() {
                    context.send_trace(message);
                }
            }
            AppMsg::RecordNexusPath(path) => {
                if let Some(context) = self.client_broker.as_context_mut()
                    && context.client.last_path != path
                {
                    context.client.last_path = path;
                    context.send_to_server(CommonRequest::Client(ClientRequest::RecordQuestEvent(
                        QuestEvent::Nexus { path },
                    )));
                }
            }
            AppMsg::VisibilityChange(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.visibility_change(event);
                }
            }
            #[cfg(feature = "pointer_lock")]
            AppMsg::PointerLockChange => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.pointer_lock_change();
                    ret = true;
                }
            }
            AppMsg::Message(message) => {
                let opt_string = message.as_string();

                let opt_str = opt_string.as_deref();
                if let Some(opt_str) = opt_str {
                    console_log!("msg: {opt_str}");
                }

                match opt_str {
                    Some("snippetLoaded") => {
                        post_message("gameLoaded");
                        if let Some(context) = self.client_broker.as_context_mut() {
                            context.client.escaping.post_message();
                        }
                    }
                    Some("disableOutbound") => {
                        self.features.outbound = OutboundLinks::NONE;
                        ret = true;
                    }
                    Some("enableOutbound") => {
                        self.features.outbound = OutboundLinks::ALL;
                        ret = true;
                    }
                    Some("disableAccounts") => {
                        self.features.outbound.accounts = Accounts::None;
                        ret = true;
                    }
                    Some("enableAccounts") => {
                        self.features.outbound.accounts = Accounts::Normal;
                        ret = true;
                    }
                    Some("disableAppStores") => {
                        self.features.outbound.app_stores = false;
                        ret = true;
                    }
                    Some("disableChat") => {
                        self.features.chat = false;
                        ret = true;
                    }
                    Some("enableChat") => {
                        self.features.chat = true;
                        ret = true;
                    }
                    Some("disablePromo") => {
                        self.features.outbound.promo = false;
                        ret = true;
                    }
                    Some("enablePromo") => {
                        self.features.outbound.promo = true;
                        ret = true;
                    }
                    Some("disableContactInfo") => {
                        self.features.outbound.contact_info = false;
                        ret = true;
                    }
                    Some("enableContactInfo") => {
                        self.features.outbound.contact_info = true;
                        ret = true;
                    }
                    Some("disableSocialMedia") => {
                        self.features.outbound.social_media = false;
                        ret = true;
                    }
                    Some("enableSocialMedia") => {
                        self.features.outbound.social_media = true;
                        ret = true;
                    }
                    Some("disableCredits") => {
                        self.features.outbound.credits = false;
                        ret = true;
                    }
                    Some("enableCredits") => {
                        self.features.outbound.credits = true;
                        ret = true;
                    }
                    Some("enableAdPrivacy") => {
                        self.features.ad_privacy = true;
                        ret = true;
                    }
                    Some("enableInvitationLinks") => {
                        self.features.outbound.invitations = InvitationLinks::Normal;
                        ret = true;
                    }
                    Some("disableInvitationLinks") => {
                        self.features.outbound.invitations = InvitationLinks::None;
                        ret = true;
                    }
                    Some(s) if s.starts_with("invitationLinkTemplate=") => {
                        let template = s.split_once('=').unwrap().1;
                        self.features.outbound.invitations =
                            InvitationLinks::Template(Rc::from(template));
                        ret = true;
                    }
                    Some(s) if s.starts_with("verbatimInvitationLink=") => {
                        let verbatim = s.split_once('=').unwrap().1;
                        self.features.outbound.invitations =
                            InvitationLinks::Verbatim(Rc::from(verbatim));
                        ret = true;
                    }
                    Some(s)
                        if s.starts_with("invitationId=")
                            || s.starts_with("acceptedInvitationId=") =>
                    {
                        let string = s.split_once('=').unwrap().1;
                        if let Ok(invitation_id) = InvitationId::from_str(string)
                            && let Some(context) = self.client_broker.as_context_mut()
                            && context.client.escaping.is_spawning()
                        {
                            let invitation_server_id = ServerId {
                                number: invitation_id.server_number(),
                                kind: context
                                    .common_settings
                                    .server_id
                                    .map(|s| s.kind)
                                    .unwrap_or(ServerKind::Cloud),
                            };
                            if invitation_server_id.kind.is_local()
                                || context.common_settings.server_id == Some(invitation_server_id)
                                || context
                                    .state
                                    .core
                                    .servers
                                    .iter()
                                    .any(|((s, _), _)| *s == invitation_server_id)
                                || context
                                    .system_info
                                    .as_ref()
                                    .map(|s| s.available_servers.contains(&invitation_server_id))
                                    .unwrap_or(false)
                            {
                                context.choose_server_id(
                                    Some(invitation_server_id),
                                    ArenaQuery::Invitation(invitation_id),
                                );
                            }
                        }
                        ret = true;
                    }
                    // Snippet can send this after it learns of a created invitation ID.
                    Some("defaultPlayWithFriendsOnPublicServer") => {
                        if let Some(context) = self.client_broker.as_context_mut() {
                            let created_invitation_id = context.state.core.created_invitation_id;
                            if let Some(created_invitation_id) = created_invitation_id
                                && context.state.core.accepted_invitation_id.is_none() {
                                // We could maintain this state on the client, but informing the server
                                // is better for metrics/quests.
                                context.send_request(CommonRequest::Invitation(InvitationRequest::Accept(Some(created_invitation_id))));
                            }
                        }
                    }
                    Some(s) if s.starts_with("enableSignInWith=") => {
                        let string = s.split_once('=').unwrap().1;
                        self.features.outbound.accounts = Accounts::Snippet {
                            provider: string.to_owned(),
                            sign_out: true,
                        };
                        ret = true;
                    }
                    Some("disableSignOut") => {
                        if let Accounts::Snippet { sign_out, .. } =
                            &mut self.features.outbound.accounts
                        {
                            *sign_out = false;
                            ret = true;
                        }
                    }
                    Some("disableCookies") => {
                        change_common_settings(
                            self.features.cookie_consent,
                            &mut self.client_broker,
                            Box::new(|settings, browser_storages| {
                                if !settings.cookie_notice_dismissed {
                                    settings.set_cookie_notice_dismissed(true, browser_storages);
                                    settings.set_statistic_cookies(false, browser_storages);
                                    settings.set_preference_cookies(false, browser_storages);
                                }
                            }),
                        );
                        ret = true;
                    }
                    #[cfg(feature = "audio")]
                    Some("mute") => {
                        if let Some(context) = self.client_broker.as_context_mut() {
                            context.audio.set_muted_by_ad(true);
                        }
                    }
                    #[cfg(feature = "audio")]
                    Some("unmute") => {
                        if let Some(context) = self.client_broker.as_context_mut() {
                            context.audio.set_muted_by_ad(false);
                        }
                    }
                    Some("enableCookieConsent") => {
                        self.features.cookie_consent = true;
                        change_common_settings(
                            self.features.cookie_consent,
                            &mut self.client_broker,
                            Box::new(|_settings, _browser_storages| {
                                // No-op; just reconciliation.
                            }),
                        );
                        ret = true;
                    }
                    Some("enableAccountExemptBannerAds") => {
                        self.features.account_exempt_banner_ads = true;
                    }
                    Some("enableAccountExemptInterstitialAds") => {
                        self.features.account_exempt_interstitial_ads = true;
                    }
                    Some("enableAccountExemptRewardedAds") => {
                        self.features.account_exempt_rewarded_ads = true;
                    }
                    Some("enableBannerAds") => {
                        if matches!(self.banner_ad, BannerAd::Unavailable) {
                            self.banner_ad = BannerAd::Available {
                                request: ctx.link().callback(|_| AppMsg::RequestBannerAd),
                            };
                            ret = true;
                        }
                    }
                    Some("enableInterstitialAds") => {
                        if matches!(self.interstitial_ad, InterstitialAd::Unavailable) {
                            self.interstitial_ad = InterstitialAd::Available {
                                request: ctx.link().callback(AppMsg::RequestInterstitialAd),
                            };
                            ret = true;
                        }
                    }
                    Some("enableRewardedAds") => {
                        if matches!(self.rewarded_ad, RewardedAd::Unavailable) {
                            self.rewarded_ad = RewardedAd::Available {
                                request: ctx.link().callback(AppMsg::RequestRewardedAd),
                            };
                            if let Some(context) = self.client_broker.as_context_mut() {
                                context.enable_rewarded_ads();
                            }
                            ret = true;
                        }
                    }
                    Some("tallyBannerAd") => {
                        if let Some(context) = self.client_broker.as_context_mut() {
                            context.ad_event(AdEvent::Banner(BannerAdEvent::Show));
                        }
                    }
                    Some("tallyRewardedAd") => {
                        if let Some(context) = self.client_broker.as_context_mut() {
                            context.ad_event(AdEvent::Rewarded(VideoAdEvent::Finish));
                            if matches!(
                                self.rewarded_ad,
                                RewardedAd::Available { .. } | RewardedAd::Watching { .. }
                            ) {
                                if let RewardedAd::Watching {
                                    callback: Some(callback),
                                } = &self.rewarded_ad
                                {
                                    callback.emit(());
                                    self.rewarded_ad = RewardedAd::Available {
                                        request: ctx.link().callback(AppMsg::RequestRewardedAd),
                                    };
                                } else {
                                    self.rewarded_ad = RewardedAd::Watched {
                                        consume: ctx.link().callback(|_| AppMsg::ConsumeRewardedAd),
                                    };
                                }
                                ret = true;
                            }
                        }
                    }
                    Some("cancelInterstitialAd") => {
                        if let InterstitialAd::Watching { callback } = &self.interstitial_ad {
                            if let Some(context) = self.client_broker.as_context_mut() {
                                context.ad_event(AdEvent::Interstitial(VideoAdEvent::Cancel));
                            }
                            if let Some(callback) = callback {
                                callback.emit(());
                            }
                            self.interstitial_ad = InterstitialAd::Available {
                                request: ctx.link().callback(AppMsg::RequestInterstitialAd),
                            };
                            ret = true;
                        }
                    }
                    Some("cancelRewardedAd") => {
                        if matches!(self.rewarded_ad, RewardedAd::Watching { .. }) {
                            if let Some(context) = self.client_broker.as_context_mut() {
                                context.ad_event(AdEvent::Rewarded(VideoAdEvent::Cancel));
                            }
                            self.rewarded_ad = RewardedAd::Available {
                                request: ctx.link().callback(AppMsg::RequestRewardedAd),
                            };
                            ret = true;
                        }
                    }
                    Some("tallyInterstitialAd") => {
                        if let InterstitialAd::Watching { callback } = &self.interstitial_ad {
                            if let Some(callback) = callback {
                                callback.emit(());
                            }
                            self.interstitial_ad = InterstitialAd::Available {
                                request: ctx.link().callback(AppMsg::RequestInterstitialAd),
                            };
                            if let Some(context) = self.client_broker.as_context_mut() {
                                context.ad_event(AdEvent::Interstitial(VideoAdEvent::Finish));
                            }
                            ret = true;
                        }
                    }
                    Some("simulateDropSocket") => {
                        if let Some(context) = self.client_broker.as_context_mut() {
                            context.simulate_drop_socket();
                            ret = true;
                        }
                    }
                    Some("awaitPointerLock") => {
                        if let Some(context) = self.client_broker.as_context_mut()
                            && !context.client.escaping.is_spawning()
                        {
                            context.set_escaping(Escaping::Escaping {
                                awaiting_pointer_lock: true,
                            });
                            ret = true;
                        }
                    }
                    Some("closeProfile") => {
                        self.context_menu = None;
                        ret = true;
                    }
                    Some("signOut" | "signOutUser" | "profileDeleted") => {
                        if opt_str == Some("signOutUser")
                            && let Some(settings) = common_settings(&self.client_broker)
                            && !settings.user
                        {
                            // No-op to prevent creating two visitors.
                        } else {
                            let set_login = ctx.link().callback(|login| {
                                AppMsg::Login(SetLogin {
                                    login,
                                    alias: SetLoginAlias::Overwrite,
                                    quit: false,
                                })
                            });
                            logout(set_login, G::GAME_CONSTANTS.game_id());
                            ret = true;
                        }
                    }
                    Some("nickNameChanged") => {
                        if let Some(common_settings) = common_settings(&self.client_broker) {
                            renew_session(
                                ctx.link().callback(|login| {
                                    AppMsg::Login(SetLogin {
                                        login,
                                        alias: SetLoginAlias::Overwrite,
                                        quit: false,
                                    })
                                }),
                                common_settings.session_id,
                                G::GAME_CONSTANTS.game_id(),
                            );
                        }
                    }
                    _ => {
                        process_finish_signin(
                            &message,
                            G::GAME_CONSTANTS,
                            &self.features.outbound.accounts,
                            &ctx.link().callback(AppMsg::Login),
                        );
                    }
                }
            }
            AppMsg::Wheel(event) => {
                if let Some(client_broker) = self.client_broker.as_mut() {
                    client_broker.wheel(event);
                }
            }
        }

        // Pointer lock requests happen in frames/input handlers (above).
        #[cfg(feature = "pointer_lock")]
        if crate::is_mobile()
            && let Some(context) = self.client_broker.as_context_ref()
            && crate::pointer_locked_with_emulation() != context.mouse.pointer_locked
        {
            ctx.link().callback(|_| AppMsg::PointerLockChange).emit(());
            ret = true;
        }

        if let Some(context) = self.client_broker.as_context_mut() {
            let cheats = context.cheats();
            if cheats != self.features.cheats {
                self.features.cheats = cheats;
                ret = true;
            }
        }

        ret
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let change_common_settings_callback = ctx.link().callback(AppMsg::ChangeCommonSettings);
        let change_settings_callback = ctx.link().callback(AppMsg::ChangeSettings);
        let chat_request_callback = ctx.link().callback(AppMsg::SendChatRequest);
        let client_request_callback = ctx.link().callback(AppMsg::SendClientRequest);
        let invitation_request_callback = ctx.link().callback(AppMsg::SendInvitationRequest);
        let raw_zoom_callback = ctx.link().callback(AppMsg::RawZoom);
        let recreate_renderer_callback = ctx.link().callback(|_| AppMsg::RecreateCanvas);
        let set_server_id_callback: Callback<(ServerId, ArenaQuery)> =
            ctx.link().callback(|(s, a)| AppMsg::SetServerId(s, a));
        let send_ui_event_callback = ctx.link().callback(AppMsg::SendUiEvent);
        let set_context_menu_callback = ctx.link().callback(AppMsg::SetContextMenuProps);
        let set_escaping_callback = ctx.link().callback(AppMsg::SetEscaping);

        let setting_cache = common_settings(&self.client_broker).unwrap().clone();
        let translator = Translator {
            languages: Rc::clone(&self.translation_cache.languages),
            language_id: setting_cache.language,
            translations: self.translation_cache.get(setting_cache.language),
        };

        let pathname = browser_pathname();
        let basename = match setting_cache.arena_id.realm_id() {
            Some(RealmId::Temporary(invitation_id)) => Some(format!("/party/{invitation_id}")),
            Some(RealmId::Named(realm_name)) => Some(format!("/named/{realm_name}")),
            _ => None,
        };
        let pathname =
            pathname.trim_start_matches(basename.as_deref().unwrap_or("").trim_end_matches('/'));
        let route = Nexus::<G::UiRoute>::recognize(&pathname).unwrap_or(Nexus::NotFound);
        let nexus = route != Nexus::NotFound;
        if let Some(context) = self.client_broker.as_context_ref() {
            let path = nexus
                .then(|| route.to_path())
                .and_then(|path| NexusPath::from_str(&path).ok());
            if path != context.client.last_path {
                ctx.link().send_message(AppMsg::RecordNexusPath(path));
            }
        }
        // Combine game and engine routes, except those with path parameters.
        let route_clone = route.clone();
        let filtered_tabs: Vec<_> = Nexus::<G::UiRoute>::tabs()
            .filter(move |r| {
                !(r == &Nexus::Engine(EngineNexus::Store) && !setting_cache.store_enabled)
                    && !(r == &Nexus::Engine(EngineNexus::Profile)
                        && self.features.outbound.accounts.is_none())
                    && if route.category().is_none() || r.category().is_none() {
                        &route == r
                    } else {
                        route.category() == r.category()
                    }
            })
            .collect();
        let routes = Rc::new(
            filtered_tabs
                .into_iter()
                .map(|r| NexusRoute {
                    label: r.label(&translator),
                    route: r.to_path(),
                    selected: r == route_clone,
                })
                .collect(),
        );

        let escaping = self
            .client_broker
            .as_context_ref()
            .map(|context| context.client.escaping)
            .unwrap_or_default();

        let mut banner_ad = self.banner_ad.clone();
        let mut interstitial_ad = self.interstitial_ad.clone();
        let mut rewarded_ad = self.rewarded_ad.clone();

        let account = self.features.outbound.accounts.is_some() && setting_cache.user;
        if account && self.features.account_exempt_banner_ads {
            banner_ad = BannerAd::Unavailable;
        }
        if account && self.features.account_exempt_interstitial_ads {
            interstitial_ad = InterstitialAd::Unavailable;
        }
        if account && self.features.account_exempt_rewarded_ads {
            rewarded_ad = RewardedAd::Unavailable;
        }

        #[cfg(feature = "pointer_lock")]
        let pointer_locked = self
            .client_broker
            .as_context_ref()
            .map(|context| context.mouse.pointer_locked)
            .unwrap_or(false);

        let context = Ctw {
            escaping,
            nexus,
            #[cfg(feature = "pointer_lock")]
            pointer_locked,
            chat_request_callback,
            client_request_callback,
            invitation_request_callback,
            change_common_settings_callback,
            set_server_id_callback,
            game_constants: G::GAME_CONSTANTS,
            features: self.features.clone(),
            banner_ad,
            interstitial_ad,
            rewarded_ad,
            raw_zoom_callback,
            recreate_renderer_callback,
            set_context_menu_callback,
            set_escaping_callback,
            routes,
            licenses: G::LICENSES,
            translator,
            setting_cache,
            state: self
                .client_broker
                .as_context_ref()
                .map(|context| WeakCoreState::new(&context.state.core))
                .unwrap_or_default(),
            available_servers: self
                .client_broker
                .as_context_ref()
                .and_then(|context| {
                    context
                        .system_info
                        .as_ref()
                        .map(|s| s.available_servers.clone())
                })
                .unwrap_or_default(),
            translate_rank_number: G::translate_rank_number,
            translate_rank_benefits: G::translate_rank_benefits,
        };

        let game_context = Gctw {
            send_ui_event_callback,
            settings_cache: match &self.client_broker {
                PendingBroker::Done(client_broker) => client_broker.context.settings.clone(),
                PendingBroker::Failed { context, .. } => context.settings.clone(),
                PendingBroker::Pending { settings, .. } => settings.clone(),
                PendingBroker::Swapping => {
                    debug_assert!(false, "PendingBroker::Swapping in render");
                    G::GameSettings::default()
                }
            },
            change_settings_callback,
        };

        html! {
            <BrowserRouter basename={basename.map(AttrValue::from)}>
                <ContextProvider<Ctw> {context}>
                    <ContextProvider<Gctw<G>> context={game_context}>
                        if self.recreating_canvas != RecreatingCanvas::Started {
                            <Canvas
                                blur={escaping.is_escaping() || nexus}
                                resolution_divisor={NonZeroU8::new(1).unwrap()}
                                mouse_callback={cfg!(not(feature = "mouse_over_ui")).then(|| ctx.link().callback(AppMsg::Mouse))}
                                touch_callback={ctx.link().callback(AppMsg::Touch)}
                                focus_callback={ctx.link().callback(AppMsg::MouseFocus)}
                                wheel_callback={cfg!(not(feature = "mouse_over_ui")).then(|| ctx.link().callback(AppMsg::Wheel))}
                            />
                        }
                        if self.client_broker.as_context_ref().map(|context| context.connection_lost()).unwrap_or_default() {
                            <FatalErrorDialog/>
                        } else if let PendingBroker::Failed{error, ..} = &self.client_broker {
                            <FatalErrorDialog error={*error}/>
                        } else {
                            <>
                                if escaping.is_escaping() {
                                    <EscapeMenu<G>/>
                                }
                                <G::Ui props={self.ui_props.clone()}/>
                                <Switch<Nexus<G::UiRoute>> render={Nexus::<G::UiRoute>::render::<G>}/>
                                if let Some(context_menu) = self.context_menu.as_ref() {
                                    {context_menu.clone()}
                                }
                                if self.client_broker.as_context_ref().map(|context| context.socket.is_reconnecting()).unwrap_or_default() {
                                    <Reconnecting/>
                                }
                            </>
                        }
                    </ContextProvider<Gctw<G>>>
                </ContextProvider<Ctw>>
            </BrowserRouter>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, _first_render: bool) {
        match self.recreating_canvas {
            RecreatingCanvas::None => {}
            RecreatingCanvas::Started => ctx.link().send_message(AppMsg::RecreateCanvasPart2),
            RecreatingCanvas::Finished => ctx.link().send_message(AppMsg::RecreateRenderer),
        }
    }
}

fn common_settings<G: GameClient>(client_broker: &PendingBroker<G>) -> Option<&CommonSettings> {
    match client_broker {
        PendingBroker::Done(client_broker) => Some(&client_broker.context.common_settings),
        PendingBroker::Failed { context, .. } => Some(&context.common_settings),
        PendingBroker::Pending {
            common_settings, ..
        } => Some(common_settings),
        PendingBroker::Swapping => {
            debug_assert!(false, "PendingBroker::Swapping in common_settings");
            None
        }
    }
}

fn reconcile_cookies<G: GameClient>(
    cookie_consent: bool,
    common_settings: &CommonSettings,
    _settings: &G::GameSettings,
    browser_storages: &mut BrowserStorages,
) {
    browser_storages.set_preference_cookies(
        !cookie_consent || common_settings.preference_cookies,
        CommonSettings::preferences()
            .iter()
            .chain(G::GameSettings::preferences())
            .copied(),
    );
    browser_storages.set_statistic_cookies(
        !cookie_consent || common_settings.statistic_cookies,
        CommonSettings::statistics()
            .iter()
            .chain(G::GameSettings::statistics())
            .copied(),
    );
}

fn settings_mut<G: GameClient>(
    client_broker: &mut PendingBroker<G>,
) -> Option<(
    &mut CommonSettings,
    &mut G::GameSettings,
    &mut BrowserStorages,
)> {
    match client_broker {
        PendingBroker::Done(client_broker) => Some((
            &mut client_broker.context.common_settings,
            &mut client_broker.context.settings,
            &mut client_broker.context.browser_storages,
        )),
        PendingBroker::Failed { context, .. } => Some((
            &mut context.common_settings,
            &mut context.settings,
            &mut context.browser_storages,
        )),
        PendingBroker::Pending {
            common_settings,
            settings,
            browser_storages,
            ..
        } => Some((common_settings, settings, browser_storages)),
        PendingBroker::Swapping => {
            debug_assert!(false, "PendingBroker::Swapping in settings_mut");
            None
        }
    }
}

fn change_common_settings<G: GameClient>(
    cookie_consent: bool,
    client_broker: &mut PendingBroker<G>,
    change: Box<dyn FnOnce(&mut CommonSettings, &mut BrowserStorages)>,
) {
    if let Some((common_settings, settings, browser_storages)) = settings_mut(client_broker) {
        change(common_settings, browser_storages);
        reconcile_cookies::<G>(
            cookie_consent,
            &*common_settings,
            &*settings,
            browser_storages,
        );
    }
}

fn change_settings<G: GameClient>(
    client_broker: &mut PendingBroker<G>,
    change: Box<dyn FnOnce(&mut G::GameSettings, &mut BrowserStorages)>,
) {
    if let Some((_, settings, browser_storages)) = settings_mut(client_broker) {
        change(settings, browser_storages);
    }
}

pub fn entry_point<G: GameClient>() {
    #[cfg(feature = "log")]
    let _ = console_log::init_with_level(log::Level::Debug);

    yew::Renderer::<App<G>>::new().render();
}

#[derive(PartialEq, Clone)]
pub enum Nexus<G> {
    Engine(EngineNexus),
    Game(G),
    NotFound,
}

impl<G: RoutableExt> RoutableExt for Nexus<G> {
    fn label(&self, t: &Translator) -> String {
        match self {
            Self::Engine(engine) => engine.label(t),
            Self::Game(game) => game.label(t),
            Self::NotFound => {
                debug_assert!(false);
                "Not found".to_string()
            }
        }
    }

    fn render<G2: GameClient>(self) -> Html {
        match self {
            Self::Engine(engine) => engine.render::<G2>(),
            Self::Game(game) => game.render::<G2>(),
            Self::NotFound => Html::default(),
        }
    }

    fn category(&self) -> Option<&'static str> {
        match self {
            Self::Engine(engine) => engine.category(),
            Self::Game(game) => game.category(),
            Self::NotFound => None,
        }
    }

    fn tabs() -> impl Iterator<Item = Self> + 'static {
        G::tabs()
            .map(Self::Game)
            .chain(EngineNexus::tabs().map(Self::Engine))
    }
}

impl<G: Routable> Routable for Nexus<G> {
    fn from_path(path: &str, params: &HashMap<&str, &str>) -> Option<Self> {
        EngineNexus::from_path(path, params)
            .map(Self::Engine)
            .or_else(|| G::from_path(path, params).map(Self::Game))
    }

    fn not_found_route() -> Option<Self> {
        assert!(EngineNexus::not_found_route().is_none());
        assert!(G::not_found_route().is_none());
        Some(Self::NotFound)
    }

    fn recognize(pathname: &str) -> Option<Self> {
        EngineNexus::recognize(pathname)
            .map(Self::Engine)
            .or_else(|| G::recognize(pathname).map(Self::Game))
            .or_else(|| Self::not_found_route())
    }

    fn routes() -> Vec<&'static str> {
        let mut ret = G::routes();
        ret.append(&mut EngineNexus::routes());
        ret.push("/");
        ret
    }

    fn to_path(&self) -> String {
        match self {
            Self::Engine(engine) => engine.to_path(),
            Self::Game(game) => game.to_path(),
            Self::NotFound => "/".to_owned(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct NexusRoute {
    pub label: String,
    pub route: String,
    pub selected: bool,
}

#[derive(Clone, Copy, PartialEq, SmolRoutable)]
pub enum Route {
    #[at("/invite/:invitation_id/")]
    Invitation { invitation_id: InvitationId },
    #[at("/realm/:realm_name/")]
    Realm { realm_name: RealmName },
    #[at("/party/:invitation_id/")]
    Temporary { invitation_id: InvitationId },
    #[at("/referrer/:referrer/")]
    Referrer { referrer: Referrer },
    #[not_found]
    #[at("/")]
    Home,
}

#[derive(Clone, Copy, PartialEq, SmolRoutable)]
pub enum EngineNexus {
    #[at("/ranks/")]
    Ranks,
    #[at("/feedback/")]
    Feedback,
    #[at("/play-with-friends/")]
    PlayWithFriends,
    #[at("/profile/")]
    Profile,
    #[at("/store/")]
    Store,
    #[at("/settings/")]
    Settings,
    #[at("/privacy/")]
    Privacy,
    #[at("/terms/")]
    Terms,
    #[at("/licensing/")]
    Licensing,
    //#[not_found]
    //#[at("/")]
    //Home,
}

pub trait RoutableExt: Routable + 'static {
    fn label(&self, t: &Translator) -> String;
    fn render<G: GameClient>(self) -> Html;
    fn category(&self) -> Option<&'static str> {
        None
    }
    fn tabs() -> impl Iterator<Item = Self> + 'static;
}

impl RoutableExt for EngineNexus {
    fn label(&self, t: &Translator) -> String {
        match self {
            Self::Ranks => translate!(t, "Ranks"),
            Self::Feedback => t.feedback_label(),
            // TODO: none should be *_hint and ideally all should be *_label.
            Self::PlayWithFriends => translate!(t, "Play with friends"),
            Self::Licensing => translate!(t, "Licensing"),
            Self::Privacy => translate!(t, "Privacy"),
            Self::Profile => t.profile_label(),
            Self::Settings => t.settings_title(),
            Self::Store => translate!(t, "Store"),
            Self::Terms => translate!(t, "Terms"),
        }
    }

    fn render<G: GameClient>(self) -> Html {
        match self {
            Self::Ranks => html! {
                <RanksDialog/>
            },
            Self::Feedback => html! {
                <FeedbackDialog/>
            },
            Self::PlayWithFriends => html! {
                <PlayWithFriendsDialog<G>/>
            },
            Self::Profile => html! {
                <ProfileDialog/>
            },
            Self::Store => html! {
                <StoreDialog/>
            },
            Self::Settings => html! {
                <SettingsDialog<G>/>
            },
            Self::Privacy => html! {
                <PrivacyDialog/>
            },
            Self::Terms => html! {
                <TermsDialog/>
            },
            Self::Licensing => html! {
                <LicensingDialog/>
            },
        }
    }

    fn category(&self) -> Option<&'static str> {
        match self {
            Self::PlayWithFriends => Some("games"),
            Self::Settings | Self::Profile | Self::Store => Some("settings"),
            //Self::Licensing => Some("help"),
            _ => None,
        }
    }

    fn tabs() -> impl Iterator<Item = Self> + 'static {
        [
            Self::Feedback,
            Self::PlayWithFriends,
            Self::Licensing,
            Self::Profile,
            Self::Store,
            Self::Settings,
            Self::Privacy,
            Self::Terms,
        ]
        .into_iter()
    }
}
