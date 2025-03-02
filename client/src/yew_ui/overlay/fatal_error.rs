// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    translate, use_change_common_settings_callback, use_translator, ArenaQuery, FatalError,
    Position, Positioner,
};
use stylist::yew::styled_component;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{window, Request, RequestInit, RequestMode, Response};
use yew::{classes, html, use_state, Html, Properties};

#[derive(Properties, PartialEq)]
pub struct FatalErrorProps {
    #[prop_or(None)]
    pub error: Option<FatalError>,
}

#[styled_component(FatalErrorDialog)]
pub fn fatal_error(props: &FatalErrorProps) -> Html {
    let container_style = css!(
        r#"
        background-color: #f6f6f6;
		border-radius: 1rem;
		box-shadow: 0em 0.25rem 0 #cccccc;
		color: #000000;
		word-break: break-word;
        "#
    );

    let p_css = css!(
        r#"
        font-size: 1.5rem;
        margin: 1rem;
        "#
    );

    let small_css = css!(
        r#"
        font-size: 1rem;
        display: block;
        margin: 1rem;
        "#
    );

    let button_css = css! {
        r#"
        background-color: #549f57;
        border-radius: 1rem;
        border: 1px solid #61b365;
        box-sizing: border-box;
        color: white;
        cursor: pointer;
        font-size: 2rem;
        margin: 1rem;
        min-width: 12rem;
        padding-bottom: 0.7rem;
        padding-top: 0.5rem;
        text-decoration: none;
        white-space: nowrap;
        width: min-content;

        :disabled {
            filter: opacity(0.6);
        }

        :hover:not(:disabled) {
            filter: brightness(0.95);
        }

        :active:not(:disabled) {
            filter: brightness(0.9);
        }
        "#
    };

    let status = use_state::<Option<&'static str>, _>(|| None);
    let change_common_settings_callback = use_change_common_settings_callback();

    // Refresh the page, which serves multiple purposes:
    // - The server may have restarted, so might need to download new client
    // - The refreshed client will attempt to regain connection
    // - The server may have gone away, so might need to connect to new server
    let refresh = {
        let status = status.clone();
        let change_common_settings_callback = change_common_settings_callback.clone();
        move |_| {
            let status = status.clone();
            let change_common_settings_callback = change_common_settings_callback.clone();
            status.set(Some("Connecting..."));
            let _ = future_to_promise(async move {
                // Do a pre-flight request to make sure we aren't refreshing ourselves into a browser error.
                let opts = RequestInit::new();
                opts.set_method("GET");
                opts.set_mode(RequestMode::Cors);

                let request = match Request::new_with_str_and_init("/", &opts) {
                    Ok(request) => request,
                    Err(_) => return Err(JsValue::NULL),
                };
                let window = window().unwrap();
                let response_value = match JsFuture::from(window.fetch_with_request(&request)).await
                {
                    Ok(response_value) => response_value,
                    Err(_) => {
                        status.set(Some(
                            "Connection failed due to lack of internet or temporary server issue.",
                        ));
                        return Err(JsValue::NULL);
                    }
                };
                let response: Response = match response_value.dyn_into() {
                    Ok(response) => response,
                    Err(_) => return Err(JsValue::NULL),
                };
                if response.ok() {
                    status.set(Some("Connected, reloading..."));
                    change_common_settings_callback.emit(Box::new(
                        move |common_settings, browser_storage| {
                            // We might be here on account of invalid realm id.
                            common_settings.set_server_id(None, browser_storage);
                            common_settings.set_arena_id(ArenaQuery::default(), browser_storage);
                        },
                    ));
                    let _ = window.location().reload();
                } else {
                    status.set(Some("Connection failed to to server error or rate limit."));
                }
                Ok(JsValue::NULL)
            });
        }
    };

    let t = use_translator();

    let message = if let Some(error) = props.error {
        t.fatal_error(error)
    } else {
        translate!(
            t,
            "connection_lost_message",
            "Lost connection to server. Try again later!"
        )
    };

    html! {
        <Positioner id="fatal_error" position={Position::Center} class={classes!(container_style)}>
            <p class={p_css}>{message}</p>
            <button onclick={refresh} class={button_css}>{"Refresh"}</button>
            if let Some(status) = *status {
                <p class={small_css}>{status}</p>
            }
        </Positioner>
    }
}
