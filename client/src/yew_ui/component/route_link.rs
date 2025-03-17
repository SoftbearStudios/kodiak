// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use stylist::yew::styled_component;
use web_sys::MouseEvent;
use yew::{classes, hook, html, AttrValue, Callback, Children, Classes, Html, Properties};
use yew_router::hooks::use_navigator;
use yew_router::Routable;

#[derive(PartialEq, Properties)]
pub struct RouteLinkProps<R: Routable> {
    #[prop_or(None)]
    pub title: Option<AttrValue>,
    pub children: Children,
    pub route: R,
    #[prop_or(None)]
    pub style: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

#[hook]
pub fn use_navigation<R: Routable + 'static>(route: R) -> Callback<MouseEvent> {
    let navigator = use_navigator().unwrap();
    Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        e.stop_propagation();
        navigator.push(&route);
    })
}

#[styled_component(RouteLink)]
pub fn route_link<R: Routable + Clone + 'static>(props: &RouteLinkProps<R>) -> Html {
    let style = css!(
        r#"
        color: white;
        cursor: pointer;
        user-select: none;
        user-drag: none;
        -webkit-user-drag: none;
        "#
    );

    let onclick = use_navigation(props.route.clone());
    let href = props.route.to_path();

    html! {
        <a
            {href}
            {onclick}
            title={props.title.clone()}
            style={props.style.clone()}
            class={classes!(style, props.class.clone())}
        >
            {props.children.clone()}
        </a>
    }
}
