// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::use_ctw;
use stylist::css;
use web_sys::MouseEvent;
use yew::virtual_dom::AttrValue;
use yew::{function_component, html, Html, Properties};
use yew_icons::{Icon, IconId};

#[derive(PartialEq, Properties)]
pub struct NexusButtonProps {
    #[prop_or("2rem".into())]
    pub size: AttrValue,
}

/// Toggles the <ESC> menu, which in turn provides access to the Nexus.
#[function_component(NexusButton)]
pub fn nexus_button(props: &NexusButtonProps) -> Html {
    let ctw = use_ctw();
    let onclick = ctw.escaping.toggle().map(|toggle| {
        ctw.set_escaping_callback.reform(move |event: MouseEvent| {
            event.prevent_default();
            event.stop_propagation();
            toggle
        })
    });

    let class = css!(
        r#"
        color: white;
        cursor: pointer;
        user-select: none;
        vertical-align: bottom;
        padding: 2px;
        border-radius: 0.6rem;

        :hover {
            background-color: #fff1
        }
    "#
    );
    html! {
        <Icon
            icon_id={IconId::LucideMenu}
            {onclick}
            width={props.size.clone()}
            height={props.size.clone()}
            {class}
        />
    }
}
