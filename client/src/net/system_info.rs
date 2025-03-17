// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::js_hooks::console_log;
use crate::{
    get_real_referrer, ArenaQuery, CohortId, DeepConnect, LanguageId, RealmId, Route, ServerId,
    SystemQuery, SystemResponse, TranslationRequest,
};
use std::ops::Deref;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Request, RequestInit, RequestMode, Response, Url};
use yew_router::Routable;

/// Information derived from a system request.
pub struct SystemInfo {
    pub(crate) host: String,
    pub(crate) encryption: bool,
    pub(crate) response: SystemResponse,
}

impl Deref for SystemInfo {
    type Target = SystemResponse;

    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

impl SystemInfo {
    pub(crate) async fn new(
        server_id: Option<ServerId>,
        arena_id: ArenaQuery,
        cohort_id: CohortId,
        language_id: LanguageId,
        game_domain: &'static str,
    ) -> Option<Self> {
        Self::new_inner(server_id, arena_id, cohort_id, language_id, game_domain)
            .await
            .inspect_err(|e| console_log!("system error: {}", e))
            .ok()
    }

    async fn new_inner(
        server_id: Option<ServerId>,
        arena_id: ArenaQuery,
        cohort_id: CohortId,
        language_id: LanguageId,
        game_domain: &'static str,
    ) -> Result<Self, String> {
        let query = SystemQuery {
            server_id,
            arena_id,
            cohort_id,
            translation: TranslationRequest { language_id },
            referrer: get_real_referrer(game_domain),
        };

        let query_string = serde_urlencoded::to_string(&query).unwrap();

        let url = format!("/system.json?{}", query_string);

        let response = js_fetch(&url).await?;
        let redirect_url = Url::new(&response.url()).map_err(|e| format!("{:?}", e))?;
        let text = js_response_text(response).await?;
        let decoded: SystemResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;

        Ok(Self {
            host: redirect_url.host(),
            encryption: redirect_url.protocol() != "http:",
            response: decoded,
        })
    }
}

/// Reads the `DeepConnect` present in the path, if any.
/// For example, path may resemble /invite/INVITE_CODE_HERE/
pub fn deep_connect() -> Option<DeepConnect> {
    js_location_pathname().ok().and_then(|pathname| {
        Route::recognize(&pathname).and_then(|route| match route {
            Route::Invitation { invitation_id } => Some(DeepConnect::Invitation(invitation_id)),
            Route::Realm { realm_name } => Some(DeepConnect::Realm(RealmId::Named(realm_name))),
            Route::Temporary { invitation_id } => {
                Some(DeepConnect::Realm(RealmId::Temporary(invitation_id)))
            }
            _ => None,
        })
    })
}

pub async fn js_fetch(url: &str) -> Result<Response, String> {
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);
    let request = Request::new_with_str_and_init(url, &opts).map_err(|e| format!("{:?}", e))?;
    let window = web_sys::window().unwrap();
    let js_response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{:?}", e))?;
    let response: Response = js_response.dyn_into().map_err(|e| format!("{:?}", e))?;
    Ok(response)
}

pub fn js_location_pathname() -> Result<String, String> {
    let pathname = window()
        .unwrap()
        .location()
        .pathname()
        .map_err(|e| format!("{:?}", e))?;
    Ok(pathname)
}

pub async fn js_response_text(response: Response) -> Result<String, String> {
    let json_promise = response.text().map_err(|e| format!("{:?}", e))?;
    let json: String = JsFuture::from(json_promise)
        .await
        .map_err(|e| format!("{:?}", e))?
        .as_string()
        .ok_or(String::from("JSON not string"))?;
    Ok(json)
}
