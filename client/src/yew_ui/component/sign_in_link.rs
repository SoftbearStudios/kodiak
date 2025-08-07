// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::js_hooks::window;
use crate::{
    post_message, translate, use_ctw, use_features, use_navigation, use_translator, Accounts,
    ContextMenu, Ctw, EngineNexus, GameConstants, GameId, RankNumber, SessionId, SessionToken,
    UserId, VisitorId,
};
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;
use strum::IntoEnumIterator;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    FormData, MouseEvent, Request, RequestCredentials, RequestInit, RequestMode, Response,
};
use yew::{
    function_component, hook, html, use_effect_with, use_state_eq, Callback, Html, Properties,
    UseStateHandle,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Login {
    pub session_id: SessionId,
    pub session_token: SessionToken,
    #[serde(default)]
    pub nick_name: Option<String>,
    #[serde(default)]
    pub user: bool,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub store_enabled: bool,
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

pub(crate) struct SetLogin {
    pub(crate) login: Login,
    /// Overwrite the current alias based on login. Typically `true` when login was
    /// initiated by player, and `false` if login was an automatic renewal.
    pub(crate) alias: SetLoginAlias,
    /// Quit to menu after login.
    pub(crate) quit: bool,
}

pub(crate) enum SetLoginAlias {
    /// Overwrite the current alias based on the login. Typically used for manual logins.
    Overwrite,
    /// Overwrite the current alias based on the login if it is a guest name. Typically used
    /// for snippet-based automatic login renewals.
    OverwriteGuestName,
    /// Don't change the current alias. Typically used for native automatic login renewals.
    NoEffect,
}

#[derive(PartialEq, Properties)]
pub struct SignInLinkProps {
    #[prop_or(false)]
    pub hide_login: bool,
}

pub(crate) fn process_finish_signin(
    data: &JsValue,
    game_constants: &GameConstants,
    accounts: &Accounts,
    set_login: &Callback<SetLogin>,
) {
    let url;
    let body;
    let alias;
    let quit;
    if data.is_object() {
        let pmcsrf = js_sys::Reflect::get(&data, &JsValue::from_str("pmcsrf"))
            .ok()
            .and_then(|v| v.as_string())
            .and_then(|s| u64::from_str(&s).ok());

        let session_id = js_sys::Reflect::get(&data, &JsValue::from_str("sessionId"))
            .ok()
            .and_then(|v| v.as_string())
            .and_then(|s| u64::from_str(&s).ok());

        let (pmcsrf, session_id) = if let Some(tuple) = pmcsrf.zip(session_id) {
            tuple
        } else {
            return;
        };
        url = format!("https://softbear.com/api/auth/session?sessionId={session_id}");
        body = FormData::new().unwrap();
        body.append_with_str("pmcsrf", &pmcsrf.to_string()).unwrap();
        body.append_with_str("sessionId", &session_id.to_string())
            .unwrap();
        alias = SetLoginAlias::Overwrite;
        quit = false;
    } else if let Accounts::Snippet { provider, .. } = &accounts
        && let Some(string) = data.as_string()
        && let siw = string.starts_with("signedInWith=")
        && let asiw = string.starts_with("autoSignedInWith=")
        && let qsiw = string.starts_with("quitSignedInWith=")
        && (siw || asiw || qsiw)
    {
        let token_provider = provider.to_ascii_lowercase();
        let token = string.split_once('=').unwrap().1;
        url = format!("https://softbear.com/api/auth/token?sessionId=1234");

        body = FormData::new().unwrap();
        body.append_with_str("gameId", game_constants.game_id().as_str())
            .unwrap();
        body.append_with_str("provider", &token_provider).unwrap();
        body.append_with_str("token", &token).unwrap();
        alias = if siw {
            SetLoginAlias::Overwrite
        } else {
            SetLoginAlias::OverwriteGuestName
        };
        quit = qsiw;
    } else {
        return;
    };

    let set_login = set_login.clone();

    let _ = future_to_promise(async move {
        let opts = RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(RequestMode::Cors);
        opts.set_credentials(RequestCredentials::Include);
        opts.set_body(&body);

        let request =
            Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{:?}", e))?;

        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| format!("{:?}", e))?;
        let resp: Response = resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;
        if resp.ok() {
            let json_promise = resp.text().map_err(|e| format!("{:?}", e))?;
            let json: String = JsFuture::from(json_promise)
                .await
                .map_err(|e| format!("{:?}", e))?
                .as_string()
                .ok_or(String::from("JSON not string"))?;
            let decoded: Login = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            set_login.emit(SetLogin {
                login: decoded,
                alias,
                quit,
            });
        }

        Ok(JsValue::NULL)
    });
}

#[function_component(SignInLink)]
pub fn sign_in_link(props: &SignInLinkProps) -> Html {
    let ctw = use_ctw();
    let t = use_translator();
    let features = use_features();
    let previous_session_id = ctw.setting_cache.session_id;

    /*
    let client_request_callback = use_client_request_callback();
    let change_common_settings = use_change_common_settings_callback();
    let set_login = set_login(
        client_request_callback,
        change_common_settings.clone(),
        true,
    );
    let onclick_logout = previous_session_id.map(|_| {
        let set_login = set_login.clone();
        Callback::from(move |_: MouseEvent| {
            logout(set_login.clone());
        })
    });
    */
    let onclick_profile = use_navigation(EngineNexus::Profile);

    // Trick yew into not warning about bad practice.
    let href: &'static str = "javascript:void(0)";

    let sign_in_with =
        |accounts: &Accounts| -> Option<(Cow<'static, str>, Option<Callback<MouseEvent>>)> {
            match accounts {
                Accounts::None => None,
                Accounts::Normal => Some((
                    Cow::Borrowed(""),
                    previous_session_id.map(|session_id| {
                        Callback::from(move |_: MouseEvent| {
                            let endpoint = format!(
                                "https://softbear.com/api/auth/redirect?pmcsrf={session_id}"
                            );
                            let features = "popup,left=200,top=200,width=700,height=700";
                            let _ = window().open_with_url_and_target_and_features(
                                &endpoint, "oauth2", features,
                            );
                        })
                    }),
                )),
                Accounts::Snippet { provider, .. } => Some((
                    Cow::Owned(provider.clone()),
                    previous_session_id.is_some().then(|| {
                        Callback::from(move |_: MouseEvent| {
                            post_message("requestSignInWith");
                        })
                    }),
                )),
            }
        };

    let account_sign_in_with = |platform: &str| -> String {
        if platform == "" {
            translate!(t, "Sign in")
        } else {
            translate!(t, "Sign in with {platform}")
        }
    };
    let account_profile = || -> String { t.profile_label() };

    html! {
        if !props.hide_login && let Some((sign_in_with, onclick_login)) = sign_in_with(&features.outbound.accounts) {
            if ctw.setting_cache.user {
                <a {href} onclick={onclick_profile}>
                    {account_profile()}
                    if let Some(nick_name) = ctw.setting_cache.nick_name {
                        {" ("}{nick_name}{")"}
                    }
                </a>
            } else if let Some(onclick_login) = onclick_login {
                <a {href} onclick={onclick_login}>{account_sign_in_with(&sign_in_with)}</a>
            }
        }
    }
}

#[derive(Debug, PartialEq, Deserialize)]
#[allow(unused)]
#[serde(rename_all = "camelCase")]
struct Profile {
    #[serde(default)]
    pub date_created: u64,
    #[serde(default)]
    pub follower_count: usize,
    #[serde(default)]
    pub is_follower: bool,
    #[serde(default)]
    pub is_following: bool,
    #[serde(default)]
    pub moderator: bool,
    #[serde(default)]
    pub nick_name: Option<String>,
}

#[hook]
fn use_profile(user_id: Option<UserId>) -> UseStateHandle<Option<Profile>> {
    let my_session_id = use_ctw().setting_cache.session_id;

    let profile = use_state_eq(|| None);
    {
        let profile = profile.clone();
        use_effect_with(my_session_id.filter(|_| user_id.is_none()), move |_| {
            let _ = future_to_promise(async move {
                let url = if let Some(user_id) = user_id {
                    Cow::Owned(format!(
                        "https://softbear.com/api/social/profile.json?userId={}",
                        user_id.0
                    ))
                } else {
                    Cow::Borrowed("https://softbear.com/api/social/profile.json")
                };

                let opts = RequestInit::new();
                opts.set_method("GET");
                opts.set_mode(RequestMode::Cors);
                opts.set_credentials(RequestCredentials::Include);

                let request =
                    Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{:?}", e))?;

                let window = web_sys::window().unwrap();
                let resp_value = JsFuture::from(window.fetch_with_request(&request))
                    .await
                    .map_err(|e| format!("{:?}", e))?;
                let resp: Response = resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;
                if resp.ok() {
                    let json_promise = resp.text().map_err(|e| format!("{:?}", e))?;
                    let json: String = JsFuture::from(json_promise)
                        .await
                        .map_err(|e| format!("{:?}", e))?
                        .as_string()
                        .ok_or(String::from("JSON not string"))?;
                    let decoded: Profile =
                        serde_json::from_str(&json).map_err(|e| e.to_string())?;
                    profile.set(Some(decoded));
                }
                Ok(JsValue::NULL)
            });
        });
    }
    profile
}

pub(crate) fn logout(set_login: Callback<Login>, game_id: GameId) {
    renew_session(set_login, None, game_id);
}

pub fn profile_factory(
    ctw: &Ctw,
) -> impl Fn(Option<VisitorId>) -> Option<Callback<MouseEvent>> + Clone {
    let language_id = ctw.setting_cache.language;
    let game_constants = ctw.game_constants;
    let session_id = ctw.setting_cache.session_id;
    let ranks = RankNumber::iter()
        .map(|n| Cow::Owned((ctw.translate_rank_number)(&ctw.translator, n)))
        .intersperse(Cow::Borrowed(","))
        .collect::<String>();
    let set_context_menu_callback = ctw.set_context_menu_callback.clone();
    move |visitor_id: Option<VisitorId>| -> Option<Callback<MouseEvent>> {
        let set_context_menu_callback = set_context_menu_callback.clone();
        let ranks = ranks.clone();
        session_id.map(move |session_id| Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_propagation();
            set_context_menu_callback.emit(Some(html! {
                <ContextMenu position={&e}>
                    <iframe
                        style="border: 0;"
                        src={format!(
                            "https://softbear.com/brief/?gameId={}&languageId={language_id}&ranks={ranks}&sessionId={}&hideNav{}",
                            game_constants.game_id,
                            session_id.0,
                            visitor_id.map(|visitor_id| format!("&visitorId={visitor_id}")).unwrap_or_default()
                        )}
                    />
                </ContextMenu>
            }));
        }))
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn renew_session(set_login: Callback<Login>, renew: Option<SessionId>, game_id: GameId) {
    let url = format!(
        "https://softbear.com/api/auth/session.json?sessionId={}&gameId={game_id}",
        renew
            .map(|s| s.to_string())
            .unwrap_or_else(|| "1".to_owned()),
    );

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);
    opts.set_credentials(RequestCredentials::Include);

    let window = web_sys::window().unwrap();

    let _ = future_to_promise(async move {
        let request =
            Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{:?}", e))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| format!("{:?}", e))?;
        let resp: Response = resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;
        if resp.ok() {
            let json_promise = resp.text().map_err(|e| format!("{:?}", e))?;
            let json: String = JsFuture::from(json_promise)
                .await
                .map_err(|e| format!("{:?}", e))?
                .as_string()
                .ok_or(String::from("JSON not string"))?;
            let decoded: Login = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            set_login.emit(decoded);
        }

        Ok(JsValue::NULL)
    });
}
