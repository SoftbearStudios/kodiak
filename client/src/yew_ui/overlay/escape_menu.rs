// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    translate, use_client_request_callback, use_ctw, use_translator, ClientRequest, Curtain,
    Escaping, Flex, GameClient, Position, Positioner,
};
use stylist::yew::styled_component;
use web_sys::MouseEvent;
use yew::{html, Callback, Html};
use yew_icons::{Icon, IconId};
use yew_router::prelude::use_navigator;
use yew_router::AnyRoute;

/// The <ESC> menu provides access to the Nexus.
#[styled_component(EscapeMenu)]
pub(crate) fn escape_menu<G: GameClient>() -> Html {
    let ctw = use_ctw();
    let t = use_translator();

    let button_style = css!(
        r#"
        background-color: #161616;
        border: 2px solid #454545;
        box-sizing: border-box;
        color: white;
        cursor: pointer;
        font-size: 2rem;
        font-style: italic;
        margin-top: 0.5em;
        min-width: 12rem;
        padding-bottom: 0.5rem;
        padding-top: 0.4rem;
        text-decoration: none;
        transform: skew(12deg);
        white-space: nowrap;
        width: 100%;

        border-left: 5px solid #454545;

        :disabled {
            filter: brightness(0.8);
            cursor: initial;
        }

        :hover:not(:disabled) {
            background-color: #212121;
            border-color: #b8b8b8;
            /*border-left: 10px solid #b8b8b8;
            transition: border-left 0.15s;*/
        }

        :active:not(:disabled) {
            filter: brightness(0.85);
        }
    "#
    );

    let onclick_back = ctw.set_escaping_callback.reform(|event: MouseEvent| {
        event.prevent_default();
        event.stop_propagation();
        //#[cfg(feature = "pointer_lock")]
        //crate::request_pointer_lock_with_emulation();
        Escaping::InGame
    });
    let onclick_route_factory = {
        let navigator = use_navigator().unwrap();

        move |route: &'static str| {
            let navigator = navigator.clone();
            Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                e.stop_propagation();
                navigator.push(&AnyRoute::new(route));
            })
        }
    };
    let onclick_leave = {
        use_client_request_callback().reform(|event: MouseEvent| {
            event.prevent_default();
            event.stop_propagation();
            ClientRequest::Quit
        })
    };

    #[cfg(feature = "pointer_lock")]
    let icon_id: fn() -> IconId = || IconId::LucideMousePointerClick;
    #[cfg(not(feature = "pointer_lock"))]
    let icon_id: fn() -> IconId = || unreachable!();

    html! {
        if ctw.escaping.is_escaping_awaiting_pointer_lock() {
            <Curtain onclick={onclick_back}>
                <Positioner
                    id="escape_menu"
                    position={Position::Center}
                    flex={Flex::Column}
                >
                    <Icon
                        icon_id={icon_id()}
                        width={"3rem"}
                        height={"3rem"}
                        style={"margin: auto;"}
                    />
                    <h2 style={"margin: 0.5rem;"}>{translate!(t, "Click anywhere to enter game")}</h2>
                </Positioner>
            </Curtain>
        } else {
            <Positioner
                id="escape_menu"
                position={Position::Center}
                flex={Flex::Column}
            >
                <button
                    class={button_style.clone()}
                    onclick={onclick_back}
                >{t.resume_hint()}</button>
                <button
                    class={button_style.clone()}
                    onclick={onclick_route_factory("/help/")}
                >{t.help_hint()}</button>
                <button
                    class={button_style.clone()}
                    onclick={onclick_route_factory("/settings/")}
                >{t.settings_title()}</button>
                <button
                    class={button_style}
                    onclick={onclick_leave}
                >{t.quit_hint()}</button>
            </Positioner>
        }
    }
}
