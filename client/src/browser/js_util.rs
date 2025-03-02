// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::js_hooks::{document, window};
use crate::Referrer;
use std::cell::LazyCell;

pub fn browser_pathname() -> String {
    window()
        .location()
        .pathname()
        .unwrap_or_else(|_| String::from("/"))
}

/// e.g. foo.mk48.io
/// This is a problematic API, since it won't handle redirects.
pub fn host() -> String {
    window().location().host().unwrap()
}

/// Gets the string, ws or wss, for the websocket protocol to use.
/// This is a problematic API because it does not respect redirect schemes.
pub fn is_https() -> bool {
    window()
        .location()
        .protocol()
        .map(|p| p != "http:")
        .unwrap_or(true)
}

/// Returns `true` if the user agent is a mobile browser (may overlook some niche platforms).
pub fn is_mobile() -> bool {
    thread_local! {
        static IS_MOBILE: LazyCell<bool> = LazyCell::new(|| {
            let user_agent = window().navigator().user_agent();
            user_agent
                .map(|user_agent| {
                    ["iPhone", "iPad", "iPod", "Android"]
                        .iter()
                        .any(|platform| user_agent.contains(platform))
                })
                .unwrap_or(false)
        });
    }
    IS_MOBILE.with(|is_mobile| **is_mobile)
}

thread_local! {
    static IS_DAYTIME_TIMEZONE_OFFSET: LazyCell<(bool, i16)> = LazyCell::new(|| {
        let date = js_sys::Date::new_0();
        (
            (7..=18).contains(&date.get_hours()),
            date.get_timezone_offset() as i16,
        )
    });
}

/// As opposed to night. The intended use is reducing brightness
/// values to reduce eye strain at night.
pub fn is_daytime() -> bool {
    IS_DAYTIME_TIMEZONE_OFFSET.with(|lazy| lazy.0)
}

/// Minutes relative to UTC.
pub fn timezone_offset() -> i16 {
    IS_DAYTIME_TIMEZONE_OFFSET.with(|lazy| lazy.1)
}

#[cfg(feature = "pointer_lock")]
static MOBILE_POINTER_LOCK_EMULATION: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[cfg(feature = "pointer_lock")]
pub fn request_pointer_lock_with_emulation() {
    if is_mobile() {
        MOBILE_POINTER_LOCK_EMULATION.store(true, std::sync::atomic::Ordering::Relaxed);
        crate::js_hooks::request_fullscreen();
        crate::js_hooks::request_landscape();
    } else {
        crate::js_hooks::request_pointer_lock();
    }
}

#[cfg(feature = "pointer_lock")]
pub fn exit_pointer_lock_with_emulation() {
    if is_mobile() {
        MOBILE_POINTER_LOCK_EMULATION.store(false, std::sync::atomic::Ordering::Relaxed);
        //crate::js_hooks::exit_fullscreen();
    } else {
        crate::js_hooks::exit_pointer_lock();
    }
}

#[cfg(feature = "pointer_lock")]
pub fn pointer_locked_with_emulation() -> bool {
    if is_mobile() {
        MOBILE_POINTER_LOCK_EMULATION.load(std::sync::atomic::Ordering::Relaxed)
    } else {
        crate::js_hooks::pointer_locked()
    }
}

/// Gets the HTTP referrer.
pub fn referrer() -> Option<Referrer> {
    Referrer::new(&document().referrer())
}

pub fn ws_protocol(encrypted: bool) -> &'static str {
    if encrypted {
        "wss"
    } else {
        "ws"
    }
}
