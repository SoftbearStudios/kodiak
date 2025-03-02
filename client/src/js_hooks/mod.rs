// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![warn(missing_docs)]
//! # Js Hooks
//!
//! [`js_hooks`][`crate`] is a collection of utilities for a WASM application in a JavaScript environment.

use js_sys::Reflect;
use std::fmt;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlCanvasElement, OrientationLockType, Window};

/// Gets the window.
pub fn window() -> Window {
    web_sys::window().expect("no window")
}

/// Gets the document.
pub fn document() -> Document {
    window().document().expect("no document")
}

/// Gets the canvas for use with WebGL.
pub fn canvas() -> HtmlCanvasElement {
    document()
        .get_element_by_id("canvas")
        .expect("no canvas")
        .dyn_into::<HtmlCanvasElement>()
        .expect("invalid canvas")
}

/// Returns if the mouse pointer is locked.
///
/// Most users should use `kodiak_client::pointer_locked_with_emulation`.
pub fn pointer_locked() -> bool {
    document().pointer_lock_element().is_some()
}

/// Requests [`canvas`] to be pointer locked. Must call during click event, or after pointer
/// lock was entered before and not forcefully exitted.
///
/// Most users should use `kodiak_client::request_pointer_lock_with_emulation`.
pub fn request_pointer_lock() {
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = "request_pointer_lock_with_unadjusted_movement")]
        fn request_pointer_lock_with_unadjusted_movement(a: &Element);
    }

    request_pointer_lock_with_unadjusted_movement(&canvas());
}

/// Opposite of `request_pointer_lock`.
///
/// Most users should use `kodiak_client::exit_pointer_lock_with_emulation`.
pub fn exit_pointer_lock() {
    document().exit_pointer_lock();
}

/// Requests fullscreen mode activation.
pub fn request_fullscreen() {
    let _ = document().body().unwrap().request_fullscreen();
}

/// Opposite of `request_fullscreen`.
pub fn exit_fullscreen() {
    document().exit_fullscreen();
}

/// Returns `true` iff fullscreen is active.
pub fn fullscreen() -> bool {
    document().fullscreen_enabled()
}

/// Request landscape mode.
pub fn request_landscape() {
    let _ = window().screen().map(|screen| {
        screen
            .orientation()
            .lock(OrientationLockType::LandscapePrimary)
    });
}

/// Extracts an error message from a JavaScript error.
pub fn error_message(error: &JsValue) -> Option<String> {
    Reflect::get(error, &JsValue::from_str("message"))
        .as_ref()
        .ok()
        .and_then(JsValue::as_string)
}

/// Log an error to JavaScript's console. Use this instead of [`eprintln!`].
pub macro console_error {
    ($($t:tt)*) => {
        $crate::js_hooks::error_args(&format_args!($($t)*))
    }
}

/// Log to JavaScript's console. Use this instead of [`println!`].
pub macro console_log {
    ($($t:tt)*) => {
        $crate::js_hooks::log_args(&format_args!($($t)*))
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[doc(hidden)]
pub fn error_args(args: &fmt::Arguments) {
    error(&args.to_string())
}

#[doc(hidden)]
pub fn log_args(args: &fmt::Arguments) {
    log(&args.to_string())
}
