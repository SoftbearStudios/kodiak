// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    markdown, use_change_common_settings_callback, BrowserStorages, CommonSettings, EngineNexus,
    MarkdownOptions, Position, Positioner, RouteLink,
};
use std::rc::Rc;
use stylist::yew::styled_component;
use yew::{html, AttrValue, Html, Properties};

#[derive(PartialEq, Properties)]
pub struct CookieNoticeProps {
    pub position: Position,
}

#[styled_component(CookieNotice)]
pub fn cookie_notice(props: &CookieNoticeProps) -> Html {
    let change_settings = use_change_common_settings_callback();
    let choice_factory = |choice: bool| {
        change_settings.reform(move |_| {
            Box::new(
                move |settings: &mut CommonSettings, browser_storages: &mut BrowserStorages| {
                    settings.set_preference_cookies(choice, browser_storages);
                    settings.set_statistic_cookies(choice, browser_storages);
                    settings.set_cookie_notice_dismissed(true, browser_storages);
                },
            )
        })
    };

    let container_style = css!(
        r#"
        h3 {
            margin-top: 0;
            margin-bottom: 0.5rem;
        }

        p {
            margin-top: 0.5rem;
            margin-bottom: 0.5rem;
        }
        "#
    );

    let button_style = css!(
        r#"
        background-color: #4b618c;
        border-radius: 0.5rem;
        border: 1px solid #3d4e71;
        box-sizing: border-box;
        color: white;
        cursor: pointer;
        font-size: 1rem;
        padding: 0.35rem 0.25rem;
        text-decoration: none;
        white-space: nowrap;
        width: 100%;
        overflow: hidden;
        text-overflow: ellipsis;

        :disabled {
            filter: brightness(0.8);
            cursor: initial;
        }

        :hover:not(:disabled) {
            filter: brightness(0.95);
        }

        :active:not(:disabled) {
            filter: brightness(0.9);
        }
    "#
    );

    let button_container_style = css!(
        r#"
        display: flex; gap: 0.5rem;
        justify-content: space-evenly;
        flex-direction: row;

        @media (max-width: 1000px) {
            flex-direction: column-reverse;
        }
    "#
    );

    let buttons = html! {
        <div class={button_container_style.clone()}>
            <button
                onclick={choice_factory(false)}
                class={button_style.clone()}
                title={"Allow necessary cookies only"}
            >{"Necessary cookies only"}</button>
            <button
                onclick={choice_factory(true)}
                class={button_style.clone()}
                title={"Allow all cookies"}
            >{"Allow all cookies"}</button>
        </div>
    };
    let components = Box::new(move |href: &str, content: &str| match href {
        "/privacy/" => Some(html! {
            <RouteLink<EngineNexus> route={EngineNexus::Privacy}>{AttrValue::from(Rc::from(content))}</RouteLink<EngineNexus>>
        }),
        "/terms/" => Some(html! {
            <RouteLink<EngineNexus> route={EngineNexus::Terms}>{AttrValue::from(Rc::from(content))}</RouteLink<EngineNexus>>
        }),
        "buttons" => Some(buttons.clone()),
        _ => None,
    });

    html! {
        <Positioner
            id="cookie_notice"
            position={props.position}
            style={"background-color: #174479; padding: 0.5rem; border-radius: 0.5rem; overflow: hidden;"}
            class={container_style}
            max_width={"25%"}
        >
            {markdown(include_str!("../../translation/cookie_notice/en.md"), &MarkdownOptions{components, ..Default::default()})}
        </Positioner>
    }
}
