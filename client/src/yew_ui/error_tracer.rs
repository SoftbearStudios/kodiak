// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::GlobalEventListener;
use crate::js_hooks::error_message;
use js_sys::{JsString, Reflect};
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};
use wasm_bindgen::JsValue;
use web_sys::{ErrorEvent, PromiseRejectionEvent, WebGlContextEvent};
use yew::Callback;

/// Listens for various errors and forwards them to a trace handler.
pub struct ErrorTracer {
    _error_event_listener: GlobalEventListener<ErrorEvent>,
    _promise_rejection_event_listener: GlobalEventListener<PromiseRejectionEvent>,
    _webgl_context_event_listener: GlobalEventListener<WebGlContextEvent>,
}

impl ErrorTracer {
    pub fn new(trace_callback: Callback<String>) -> Self {
        let trace_callback_clone = trace_callback.clone();
        let trace_callback_clone_2 = trace_callback.clone();
        let governor = Rc::new(AtomicU8::new(5));
        let governor_clone = Rc::clone(&governor);
        let governor_clone_2 = Rc::clone(&governor);
        Self {
            _error_event_listener: GlobalEventListener::new_window(
                "error",
                move |event: &ErrorEvent| {
                    if governor
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
                            val.checked_sub(1)
                        })
                        .is_ok()
                    {
                        trace_callback.emit(
                            Self::get_detailed_error_message(event)
                                .unwrap_or_else(|| event.message()),
                        );
                    }
                },
                false,
            ),
            _promise_rejection_event_listener: GlobalEventListener::new_window(
                "unhandledrejection",
                move |event: &PromiseRejectionEvent| {
                    if governor_clone
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
                            val.checked_sub(1)
                        })
                        .is_ok()
                    {
                        trace_callback_clone.emit(
                            Self::get_detailed_promise_rejection_message(event)
                                .unwrap_or_else(|| String::from("promise rejection")),
                        );
                    }
                },
                false,
            ),
            _webgl_context_event_listener: GlobalEventListener::new_window(
                "webglcontextcreationerror",
                move |event: &WebGlContextEvent| {
                    if governor_clone_2
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
                            val.checked_sub(1)
                        })
                        .is_ok()
                    {
                        trace_callback_clone_2
                            .emit(format!("WebGLContextEvent: {}", event.status_message()));
                    }
                },
                false,
            ),
        }
    }

    fn get_detailed_error_message(event: &ErrorEvent) -> Option<String> {
        let error: JsValue = event.error();
        let message = error_message(&error)?;
        Some(
            if let Some(stack) = Reflect::get(&error, &JsValue::from_str("stack"))
                .ok()?
                .as_string()
            {
                format!("{}: {}", message, stack)
            } else {
                message
            },
        )
    }

    fn get_detailed_promise_rejection_message(event: &PromiseRejectionEvent) -> Option<String> {
        let reason: JsValue = event.reason();
        if reason.is_undefined() {
            return None;
        }
        let js_string: JsString = js_sys::JSON::stringify(&reason).ok()?;
        let string: String = js_string.into();
        Some(format!("promise rejection: {}", string))
    }
}
