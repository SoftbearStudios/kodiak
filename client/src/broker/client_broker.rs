// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::net::{SocketUpdate, SystemInfo};
use crate::{
    eval_snippet, js_hooks, map_ranges, Apply, ArenaQuery, BrowserStorages, ClientContext,
    ClientRequest, ClientUpdate, CommonRequest, CommonSettings, CommonUpdate, Escaping, FatalError,
    FpsMonitor, GameClient, InvitationRequest, Key, KeyboardEvent as GameClientKeyboardEvent,
    MouseButton, MouseEvent as GameClientMouseEvent, QuestEvent, VisibilityEvent,
};
use kodiak_common::glam::{IVec2, Vec2};
use wasm_bindgen::JsCast;
use web_sys::{
    Event, FocusEvent, HtmlInputElement, KeyboardEvent, MouseEvent, Touch, TouchEvent, WheelEvent,
};
use yew::Callback;

pub struct ClientBroker<G: GameClient> {
    pub game: G,
    pub context: ClientContext<G>,
    /// Id of the [`Touch`] associated with the earliest finger to make contact with the touch
    /// screen in a gesture, used to emulate left click.
    left_touch_id: Option<i32>,
    /// Id of the [`Touch`] associated with the second earliest finger to make contact with the touch
    /// screen in a gesture, used to emulate right click.
    right_touch_id: Option<i32>,
    statistic_fps_monitor: FpsMonitor,
}

impl<G: GameClient> ClientBroker<G> {
    #[allow(clippy::result_large_err)]
    pub(crate) fn new(
        browser_storages: BrowserStorages,
        common_settings: CommonSettings,
        settings: G::GameSettings,
        socket_inbound: Callback<SocketUpdate<CommonUpdate<G::GameUpdate>>>,
        set_ui_props: Callback<G::UiProps>,
        send_ui_event: Callback<G::UiEvent>,
        system_info: Option<SystemInfo>,
    ) -> Result<Self, (ClientContext<G>, FatalError)> {
        // Don't try to catch panics if aborting (because it's useless).
        #[cfg(panic = "unwind")]
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));

        #[cfg(feature = "joined")]
        crate::joined::init();

        let mut context = ClientContext::new(
            browser_storages,
            common_settings,
            settings,
            socket_inbound,
            set_ui_props,
            send_ui_event,
            system_info,
        );

        match G::new(&mut context) {
            Ok(game) => Ok(Self {
                game,
                context,
                left_touch_id: None,
                right_touch_id: None,
                statistic_fps_monitor: FpsMonitor::new(60.0),
            }),
            Err(error) => {
                context.send_to_server(CommonRequest::Client(ClientRequest::RecordQuestEvent(
                    QuestEvent::Error { error },
                )));
                Err((context, error))
            }
        }
    }

    pub(crate) fn socket_update(&mut self, update: SocketUpdate<CommonUpdate<G::GameUpdate>>) {
        let SocketUpdate::Inbound(update) = update else {
            self.context.state.archived = true;
            return;
        };
        if self.context.state.archived {
            if matches!(
                update,
                CommonUpdate::Client(ClientUpdate::SessionCreated { .. })
            ) {
                self.context.state.reset();
            } else {
                js_hooks::console_log!("received stale socket update during redial");
                return;
            }
        }
        let mut redirect = None;

        match &update {
            &CommonUpdate::Client(ClientUpdate::SessionCreated {
                token,
                server_id,
                arena_id,
                player_id,
                date_created,
                ..
            }) => {
                js_hooks::console_log!("created session at {server_id}/{arena_id}/{player_id:?}");
                // Create an invitation so that the player doesn't have to wait for one later.
                self.context
                    .send_to_server(CommonRequest::Invitation(InvitationRequest::Create));

                let settings = &mut self.context.common_settings;
                let storages = &mut self.context.browser_storages;
                settings.set_server_id(Some(server_id), storages);
                settings.set_arena_id(
                    ArenaQuery::Specific(arena_id, Some((player_id, token))),
                    storages,
                );
                settings.set_date_created(Some(date_created), storages);

                let host = ClientContext::<G>::compute_websocket_host(
                    &self.context.common_settings,
                    &self.context.system_info,
                    self.context.referrer,
                );
                self.context.socket.reset_host(host);
            }
            CommonUpdate::Client(ClientUpdate::BootstrapSnippet(snippet)) => {
                eval_snippet(snippet);
            }
            &CommonUpdate::Client(ClientUpdate::Redirect {
                server_id,
                arena_id,
                player_id,
                token,
            }) => {
                js_hooks::console_log!("redirected to {server_id}/{arena_id}/{player_id:?}");
                redirect = Some((server_id, arena_id, player_id, token));
            }
            &CommonUpdate::Client(ClientUpdate::ClearSyncState { game_fence: _ }) => {
                /*
                self.context
                    .send_to_server(Request::Client(ClientRequest::Heartbeat {
                        game_fence: Some(game_fence),
                    }));
                */
            }
            CommonUpdate::Game(update) => {
                self.game.peek_game(update, &mut self.context);
            }
            _ => {}
        }

        self.context.state.apply(update);

        if let Some((server_id, arena_id, player_id, token)) = redirect {
            self.context.choose_server_id(
                Some(server_id),
                ArenaQuery::Specific(arena_id, Some((player_id, token))),
            );
        }
    }

    pub fn frame(&mut self, time_seconds: f32) {
        // Avoid rare visibility desync because animation frame implies visibility?
        if self.context.visibility.is_hidden() {
            let implicit = VisibilityEvent::Visible(true);
            self.game.peek_visibility(&implicit, &mut self.context);
            self.context.visibility.apply(implicit);
        }

        #[cfg(feature = "audio")]
        self.context.audio.set_volume_setting(
            self.context.common_settings.volume,
            #[cfg(feature = "music")]
            self.context.common_settings.music,
            #[cfg(not(feature = "music"))]
            false,
        );

        let elapsed_seconds = (time_seconds - self.context.client.time_seconds).clamp(0.001, 0.5);
        self.context.client.time_seconds = time_seconds;

        self.context
            .socket
            .update(&mut self.context.state, time_seconds);

        self.game.tick(elapsed_seconds, &mut self.context);

        if self.context.client_activity() != self.context.reported_activity
            || time_seconds > self.context.last_activity + 4.5
        {
            self.context.heartbeat();
        } else if let Some(fps) = self.statistic_fps_monitor.update(elapsed_seconds) {
            self.context
                .send_to_server(CommonRequest::Client(ClientRequest::TallyFps(fps)));
        }

        if let Some(session_id) = self.context.common_settings.session_id {
            self.context
                .browser_storages
                .post_buffered(elapsed_seconds, session_id);
        }
    }

    pub fn keyboard(&mut self, event: KeyboardEvent) {
        self.context.cancel_afk();
        let type_ = event.type_();

        match type_.as_str() {
            "keydown" | "keyup" => {
                let down = type_ == "keydown";

                if let Some(key) = Key::try_from_key_code(event.key_code()) {
                    // Allow down keys to be released even after entering an input.
                    if down || self.context.keyboard.is_up(key) {
                        if let Some(target) = event.target() {
                            if target.is_instance_of::<HtmlInputElement>() {
                                return;
                            }
                        }
                    }

                    let e = GameClientKeyboardEvent {
                        key,
                        ctrl: event.ctrl_key() || event.meta_key(),
                        down,
                        shift: event.shift_key(),
                        time: self.context.client.time_seconds,
                    };

                    if down {
                        // Simulate zooming.
                        match key {
                            Key::PageDown => self.raw_zoom(1.0),
                            Key::PageUp => self.raw_zoom(-1.0),
                            Key::MinusUnderscore if e.ctrl => self.raw_zoom(1.0),
                            Key::EqualsPlus if e.ctrl => self.raw_zoom(-1.0),
                            Key::Escape => {
                                if self.context.client.escaping.is_in_game() {
                                    // Escape can't be used to toggle pointer lock since
                                    // browsers consider that a security risk.
                                    self.context.set_escaping(Escaping::Escaping);
                                }
                            }
                            Key::Tab if G::TAB_TO_ESCAPE => {
                                self.toggle_escaping();
                            }
                            Key::P if G::P_TO_ESCAPE => {
                                self.toggle_escaping();
                            }
                            _ => {}
                        }
                    }

                    if e.ctrl && matches!(e.key, Key::C | Key::F | Key::R | Key::V | Key::X) {
                        // No current games use Ctrl/Command key (and it's a footgun in some).
                        return;
                    }

                    // Don't block CTRL+C, CTRL+V, etc. and Command equivalents.
                    event.prevent_default();
                    event.stop_propagation();

                    if key == Key::Escape || self.context.client.escaping.is_escaping() {
                        return;
                    }

                    self.game.peek_keyboard(&e, &mut self.context);
                    self.context.keyboard.apply(e);
                }
            }
            _ => {}
        }
    }

    pub fn keyboard_focus(&mut self, event: FocusEvent) {
        if event.type_() == "blur" {
            self.context.keyboard.reset();
        }
    }

    pub fn mouse(&mut self, event: MouseEvent) {
        self.context.cancel_afk();

        if self.context.client.escaping.is_escaping() {
            return;
        }

        // Raw mouse event.
        let mouse_event = GameClientMouseEvent::Mouse;
        self.game.peek_mouse(&mouse_event, &mut self.context);
        self.context.mouse.apply(mouse_event);

        // these prevent chat from de-focusing:
        // event.prevent_default();
        // event.stop_propagation();
        let type_ = event.type_();

        match type_.as_str() {
            "mousedown" | "mouseup" => {
                if let Some(button) = MouseButton::try_from_button(event.button()) {
                    let down = type_ == "mousedown";

                    let e = GameClientMouseEvent::Button {
                        button,
                        down,
                        time: self.context.client.time_seconds,
                    };
                    self.game.peek_mouse(&e, &mut self.context);
                    self.context.mouse.apply(e);
                }
            }
            "mousemove" => {
                self.mouse_move_real(
                    event.client_x(),
                    event.client_y(),
                    event.movement_x(),
                    event.movement_y(),
                );
            }
            "mouseleave" => {
                self.context.mouse.reset();
            }
            _ => {}
        }
    }

    pub fn mouse_focus(&mut self, event: FocusEvent) {
        if event.type_() == "blur" {
            self.context.mouse.reset();
        }
    }

    pub fn touch(&mut self, event: TouchEvent) {
        event.prevent_default();
        event.stop_propagation();
        self.context.cancel_afk();

        if self.context.client.escaping.is_escaping() {
            return;
        }

        // Raw touch event.
        let touch_event = GameClientMouseEvent::Touch;
        self.game.peek_mouse(&touch_event, &mut self.context);
        self.context.mouse.apply(touch_event);

        // Don't care what event type, just consider the current set of touches.
        let target_touches = event.target_touches();

        let mut left_touch = None;
        let mut right_touch = None;

        for idx in 0..target_touches.length() {
            let touch: Touch = match target_touches.item(idx) {
                Some(touch) => touch,
                None => {
                    debug_assert!(false);
                    continue;
                }
            };

            let identifier = touch.identifier();
            if self.left_touch_id.map(|id| id == identifier).unwrap_or(
                self.right_touch_id
                    .map(|id| id != identifier)
                    .unwrap_or(true),
            ) {
                self.left_touch_id = Some(identifier);
                left_touch = Some(touch);
            } else if self
                .right_touch_id
                .map(|id| id == identifier)
                .unwrap_or(true)
            {
                self.right_touch_id = Some(identifier);
                right_touch = Some(touch);
            }
        }

        if let Some((first, second)) = left_touch
            .as_ref()
            .map(|t| IVec2::new(t.client_x(), t.client_y()).as_vec2())
            .zip(
                right_touch
                    .as_ref()
                    .map(|t| IVec2::new(t.client_x(), t.client_y()).as_vec2()),
            )
        {
            let pinch_distance = first.distance(second);

            if let Some(previous_pinch_distance) = self.context.mouse.pinch_distance {
                let delta = 0.03 * (previous_pinch_distance - pinch_distance);
                self.raw_zoom(delta);
            }

            self.context.mouse.pinch_distance = Some(pinch_distance);
        } else {
            self.context.mouse.pinch_distance = None;
        }

        macro_rules! process_touch {
            ($touch: expr, $mouse_button: expr, $overriden_by: expr, $overrides: expr, $id: ident) => {
                if let Some(touch) = $touch {
                    if self.context.mouse.is_down($mouse_button) {
                        let x = touch.client_x();
                        let y = touch.client_y();
                        #[cfg(not(feature = "pointer_lock"))]
                        self.mouse_move(x, y);
                        #[cfg(feature = "pointer_lock")]
                        if let Some(old) = self.context.mouse.view_position
                            && self.context.mouse.pointer_locked
                        {
                            let rect = js_hooks::canvas().get_bounding_client_rect();
                            let new = Self::client_coordinate_to_view(x, y);
                            let delta = (new - old)
                                * Vec2::new(rect.width() as f32, rect.height() as f32)
                                * 0.5
                                * js_hooks::window().device_pixel_ratio() as f32;
                            self.mouse_move_real(x, y, delta.x as i32, -delta.y as i32);
                        } else {
                            self.mouse_move(x, y);
                        }
                    } else {
                        if let Some(overrides) = $overrides {
                            if self.context.mouse.is_down(overrides) {
                                let e = GameClientMouseEvent::Button {
                                    button: overrides,
                                    down: false,
                                    time: self.context.client.time_seconds,
                                };
                                self.game.peek_mouse(&e, &mut self.context);
                                self.context.mouse.apply(e);
                            }
                        }

                        if $overriden_by
                            .map(|overriden_by| !self.context.mouse.is_down(overriden_by))
                            .unwrap_or(true)
                        {
                            self.mouse_move(touch.client_x(), touch.client_y());

                            // Start new click.
                            let e = GameClientMouseEvent::Button {
                                button: $mouse_button,
                                down: true,
                                time: self.context.client.time_seconds,
                            };
                            self.game.peek_mouse(&e, &mut self.context);
                            self.context.mouse.apply(e);
                        }
                    }
                } else {
                    self.$id = None;
                    if self.context.mouse.is_down($mouse_button) {
                        // Expire old click.
                        let e = GameClientMouseEvent::Button {
                            button: $mouse_button,
                            down: false,
                            time: self.context.client.time_seconds,
                        };
                        self.game.peek_mouse(&e, &mut self.context);
                        self.context.mouse.apply(e);
                    }
                }
            };
        }

        process_touch!(
            left_touch,
            MouseButton::Left,
            Some(MouseButton::Right),
            None,
            left_touch_id
        );
        process_touch!(
            right_touch,
            MouseButton::Right,
            None,
            Some(MouseButton::Left),
            right_touch_id
        );
    }

    /// For detecting when the browser tab becomes hidden.
    pub fn visibility_change(&mut self, _: Event) {
        // Written with the intention that errors bias towards visible=true.
        let visible = js_hooks::document().visibility_state() != web_sys::VisibilityState::Hidden;
        let e = VisibilityEvent::Visible(visible);
        self.game.peek_visibility(&e, &mut self.context);
        #[cfg(feature = "audio")]
        self.context.audio.peek_visibility(&e);
        let old = self.context.visibility.is_visible();
        self.context.visibility.apply(e);
        if !visible && old {
            // If we just became hidden, the next frame won't happen so we need to do this now.
            self.context.heartbeat();
        }
    }

    /// For detecting when the browser tab becomes hidden.
    #[cfg(feature = "pointer_lock")]
    pub fn pointer_lock_change(&mut self) {
        use strum::IntoEnumIterator;

        let locked = crate::pointer_locked_with_emulation();
        if !locked {
            // If we don't do this, the events will probably be lost.
            for button in <MouseButton as IntoEnumIterator>::iter() {
                if self.context.mouse.is_up(button) {
                    continue;
                }
                let e = GameClientMouseEvent::Button {
                    button,
                    down: false,
                    time: self.context.client.time_seconds,
                };
                self.game.peek_mouse(&e, &mut self.context);
                self.context.mouse.apply(e);
            }
            for key in <Key as IntoEnumIterator>::iter() {
                if self.context.keyboard.is_up(key) {
                    continue;
                }
                let e = GameClientKeyboardEvent {
                    key,
                    ctrl: false,
                    down: false,
                    shift: false,
                    time: self.context.client.time_seconds,
                };
                self.game.peek_keyboard(&e, &mut self.context);
                self.context.keyboard.apply(e);
            }
        }
        let e = GameClientMouseEvent::PointerLock(locked);
        self.game.peek_mouse(&e, &mut self.context);
        self.context.mouse.apply(e);

        if self.context.client.escaping.is_escaping() || self.context.client.escaping.is_in_game() {
            self.context
                .set_escaping(if self.context.mouse.pointer_locked {
                    Escaping::InGame
                } else {
                    Escaping::Escaping
                });
        }
    }

    /// Creates a mouse wheel event with the given delta.
    pub fn raw_zoom(&mut self, delta: f32) {
        self.context.cancel_afk();
        let e = GameClientMouseEvent::Wheel(delta);
        self.game.peek_mouse(&e, &mut self.context);
        self.context.mouse.apply(e);
    }

    pub fn toggle_escaping(&mut self) {
        if let Some(toggle) = self.context.client.escaping.toggle() {
            self.context.set_escaping(toggle);
        }
    }

    /// Converts page position (from event) to view position (-1..1).
    fn client_coordinate_to_view(x: i32, y: i32) -> Vec2 {
        let rect = js_hooks::canvas().get_bounding_client_rect();

        Vec2::new(
            map_ranges(
                x as f32,
                rect.x() as f32..rect.x() as f32 + rect.width() as f32,
                -1.0..1.0,
                false,
            ),
            map_ranges(
                y as f32,
                rect.y() as f32 + rect.height() as f32..rect.y() as f32,
                -1.0..1.0,
                false,
            ),
        )
    }

    pub fn ui_event(&mut self, event: G::UiEvent) {
        self.game.ui(event, &mut self.context);
    }

    /// Helper to issue a mouse move event from a real mouse event. Takes client coordinates.
    fn mouse_move_real(&mut self, x: i32, y: i32, dx: i32, dy: i32) {
        self.mouse_move(x, y);
        #[cfg(not(feature = "pointer_lock"))]
        let sensitivity = 1.0;
        #[cfg(feature = "pointer_lock")]
        let sensitivity = self.context.common_settings.mouse_sensitivity;
        let e = GameClientMouseEvent::DeltaPixels(IVec2::new(dx, dy).as_vec2() * sensitivity);
        self.game.peek_mouse(&e, &mut self.context);
        self.context.mouse.apply(e);
    }

    /// Helper to issue a mouse move event. Takes client coordinates.
    fn mouse_move(&mut self, x: i32, y: i32) {
        self.context.cancel_afk();
        let view_position = Self::client_coordinate_to_view(x, y);

        let e = GameClientMouseEvent::MoveViewSpace(view_position);
        self.game.peek_mouse(&e, &mut self.context);
        self.context.mouse.apply(e);
    }

    pub fn wheel(&mut self, event: WheelEvent) {
        self.context.cancel_afk();
        if !self.context.client.escaping.is_in_game() {
            return;
        }

        // Changes to escaping make this no longer necessary.
        // If `mouse_over_ui`, don't prevent scrolling the help page, etc.
        //if cfg!(not(feature = "mouse_over_ui")) { ... }
        event.prevent_default();

        // each wheel step is 53 pixels.
        // do 0.5 or 1.0 raw zoom.
        let steps: f64 = event.delta_y() * (1.0 / 53.0);
        let sign = 1f64.copysign(steps);
        let steps = steps.abs().clamp(1.0, 2.0).floor() * sign;
        self.raw_zoom(steps as f32 * 0.5)
    }
}
