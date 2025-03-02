// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::use_features;
use stylist::yew::styled_component;
use web_sys::MouseEvent;
use yew::virtual_dom::AttrValue;
use yew::{html, Callback, Children, Html, Properties};

#[derive(PartialEq, Properties)]
pub struct LinkProps {
    #[prop_or(None)]
    pub title: Option<AttrValue>,
    #[prop_or("javascript:void(0)".into())]
    pub href: AttrValue,
    #[prop_or(None)]
    pub onclick: Option<Callback<MouseEvent>>,
    #[prop_or_default]
    pub new_tab: bool,
    pub children: Children,
    #[prop_or(None)]
    pub enabled: Option<bool>,
}

#[styled_component(Link)]
pub fn link(props: &LinkProps) -> Html {
    let class = css!(
        r#"
        color: white;
		pointer-events: all;
		"#
    );

    // Avoid unwrap_or_else.
    let outbound_enabled = props.enabled.unwrap_or(use_features().outbound.any());
    let outbound = props.href.starts_with("http");
    let target = if (props.new_tab || outbound) && outbound_enabled {
        Some(AttrValue::Static("_blank"))
    } else {
        None
    };

    html! {
        if outbound_enabled || !outbound {
            <a href={props.href.clone()} {target} onclick={props.onclick.clone()} {class} rel="noopener">{props.children.clone()}</a>
        } else {
            <span {class}>{props.children.clone()}</span>
        }
    }
}
