// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::GlobalEventListener;
use web_sys::{FocusEvent, KeyboardEvent};
use yew::Callback;

pub(crate) struct KeyboardEventsListener {
    _blur_event_listener: GlobalEventListener<FocusEvent>,
    _focus_event_listener: GlobalEventListener<FocusEvent>,
    _keydown_event_listener: GlobalEventListener<KeyboardEvent>,
    _keyup_event_listener: GlobalEventListener<KeyboardEvent>,
}

impl KeyboardEventsListener {
    pub fn new(
        keyboard_callback: Callback<KeyboardEvent>,
        focus_callback: Callback<FocusEvent>,
    ) -> Self {
        let focus_callback_clone = focus_callback.clone();
        let keyboard_callback_clone = keyboard_callback.clone();
        Self {
            _blur_event_listener: GlobalEventListener::new_window(
                "blur",
                move |event: &FocusEvent| {
                    focus_callback.emit(event.clone());
                },
                true,
            ),
            _focus_event_listener: GlobalEventListener::new_window(
                "focus",
                move |event: &FocusEvent| {
                    focus_callback_clone.emit(event.clone());
                },
                true,
            ),
            _keyup_event_listener: GlobalEventListener::new_window(
                "keyup",
                move |event: &KeyboardEvent| {
                    keyboard_callback.emit(event.clone());
                },
                true,
            ),
            _keydown_event_listener: GlobalEventListener::new_window(
                "keydown",
                move |event: &KeyboardEvent| {
                    keyboard_callback_clone.emit(event.clone());
                },
                true,
            ),
        }
    }
}
