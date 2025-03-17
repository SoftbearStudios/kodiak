// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod browser_storage;
mod js_util;
mod setting;
mod snippet;
mod visibility;

pub use self::browser_storage::BrowserStorages;
pub use self::snippet::eval_snippet;
// TODO: games only use is_mobile
pub use self::js_util::{
    browser_pathname, host, is_daytime, is_https, is_mobile, referrer, timezone_offset, ws_protocol,
};
#[cfg(feature = "pointer_lock")]
pub use self::js_util::{
    exit_pointer_lock_with_emulation, pointer_locked_with_emulation,
    request_pointer_lock_with_emulation,
};
pub use self::setting::{CommonSettings, LocalSettings, SettingCategory};
// TODO: games only use VisibilityEvent
pub use self::visibility::{VisibilityEvent, VisibilityState};
