// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::js_hooks::{self, window};
use crate::SessionId;
use std::str::FromStr;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{FormData, Request, RequestInit, RequestMode, Response, Storage};

use crate::{js_response_text, RateLimiter};

/// For interacting with the local storage and session storage APIs.
pub struct BrowserStorages {
    local: BrowserStorage,
    session: BrowserStorage,
    /// Black hole; reads and writes will always return error.
    #[doc(hidden)]
    no_op: BrowserStorage,
    preference_cookies: bool,
    statistic_cookies: bool,
    buffer: Option<FormData>,
    post_rate_limit: RateLimiter,
}

impl Default for BrowserStorages {
    fn default() -> Self {
        Self {
            local: BrowserStorage::new(window().local_storage().ok().flatten()),
            session: BrowserStorage::new(window().session_storage().ok().flatten()),
            no_op: BrowserStorage::new(None),
            preference_cookies: true,
            statistic_cookies: true,
            buffer: None,
            post_rate_limit: RateLimiter::new(0.5),
        }
    }
}

fn transfer(src: &BrowserStorage, dst: &BrowserStorage, keys: impl Iterator<Item = &'static str>) {
    for key in keys {
        if let Some(value) = src.get::<String>(key) {
            let _ = dst.set(key, Some(&value));
            let _ = src.set(key, None);
        }
    }
}

impl BrowserStorages {
    pub fn buffer(&mut self, name: &str, value: &str) {
        let buffer = self.buffer.get_or_insert_with(|| FormData::new().unwrap());
        let _ = buffer.set_with_str(name, value);
    }

    pub(crate) fn post_buffered(&mut self, elapsed_seconds: f32, session_id: SessionId) {
        self.post_rate_limit.update(elapsed_seconds);
        if self.buffer.is_none() || !self.post_rate_limit.ready() {
            return;
        }
        let buffer = self.buffer.take().unwrap();
        let _ = future_to_promise(async move {
            let closure = move || async move {
                let opts = RequestInit::new();
                opts.set_method("POST");
                opts.set_mode(RequestMode::Cors);
                opts.set_body(&buffer);
                let request = Request::new_with_str_and_init(
                    &format!("https://softbear.com/api/auth/settings?sessionId={session_id}"),
                    &opts,
                )
                .map_err(|e| format!("{:?}", e))?;
                let window = web_sys::window().unwrap();
                let js_response = JsFuture::from(window.fetch_with_request(&request))
                    .await
                    .map_err(|e| format!("{:?}", e))?;
                let response: Response = js_response.dyn_into().map_err(|e| format!("{:?}", e))?;
                let _ = js_response_text(response)
                    .await
                    .map_err(|e| format!("{e:?}"))?;
                Result::<(), String>::Ok(())
            };

            if let Err(e) = closure().await {
                js_hooks::console_log!("post buffered settings: {e}");
            }

            Ok(JsValue::UNDEFINED)
        });
    }

    pub(crate) fn set_preference_cookies(
        &mut self,
        preference_cookies: bool,
        preferences: impl Iterator<Item = &'static str>,
    ) {
        match (self.preference_cookies, preference_cookies) {
            (false, true) => {
                transfer(&self.session, &self.local, preferences);
            }
            (true, false) => {
                transfer(&self.local, &self.session, preferences);
            }
            _ => {}
        }
        self.preference_cookies = preference_cookies
    }

    pub(crate) fn set_statistic_cookies(
        &mut self,
        statistic_cookies: bool,
        statistics: impl Iterator<Item = &'static str>,
    ) {
        match (self.statistic_cookies, statistic_cookies) {
            (false, true) => {
                transfer(&self.session, &self.local, statistics);
            }
            (true, false) => {
                transfer(&self.local, &self.session, statistics);
            }
            _ => {}
        }
        self.statistic_cookies = statistic_cookies
    }

    pub fn essential_storage(&self) -> &BrowserStorage {
        &self.local
    }

    pub fn preference_storage(&self) -> &BrowserStorage {
        if self.preference_cookies {
            &self.local
        } else {
            &self.session
        }
    }

    pub fn statistic_storage(&self) -> &BrowserStorage {
        if self.statistic_cookies {
            &self.local
        } else {
            &self.session
        }
    }

    pub fn volatile_storage(&self) -> &BrowserStorage {
        &self.session
    }

    pub fn no_storage(&self) -> &BrowserStorage {
        &self.no_op
    }
}

/// For interacting with the web storage API.
pub struct BrowserStorage {
    inner: Option<Storage>,
}

/// Errors that can occur with storages.
#[derive(Debug)]
pub enum Error {
    /// Javascript error.
    Js,
    /// Serialization error.
    FromStr,
    /// Storage API is not available.
    Nonexistent,
}

impl BrowserStorage {
    /// If storage API is unavailable, future calls will return `Err(Error::Nonexistent)`.
    pub(crate) fn new(inner: Option<Storage>) -> Self {
        Self { inner }
    }

    /// Gets a key from storage, returning None if it doesn't exist or any error occurs.
    pub fn get<V: FromStr>(&self, key: &str) -> Option<V> {
        self.try_get(key).ok().flatten()
    }

    /// Gets a key from storage, returning Ok(None) if it doesn't exist or Err if an error occurs.
    fn try_get<V: FromStr>(&self, key: &str) -> Result<Option<V>, Error> {
        self.inner
            .as_ref()
            .ok_or(Error::Nonexistent)?
            .get(key)
            .map_err(|_| Error::Js)?
            .map(|s| V::from_str(&s).map_err(|_| Error::FromStr))
            .transpose()
    }

    /// Sets a key in storage to a value.
    pub fn set(&self, key: &str, value: Option<&str>) -> Result<(), Error> {
        let inner = self.inner.as_ref().ok_or(Error::Nonexistent)?;
        match value {
            Some(v) => inner.set(key, v),
            None => inner.delete(key),
        }
        .map_err(|_| Error::Js)
    }
}
