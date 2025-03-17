// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::js_hooks;
use stylist::yew::styled_component;
use web_sys::{window, MouseEvent};
use yew::virtual_dom::AttrValue;
use yew::{html, Callback, Html, Properties};
use yew_icons::{Icon, IconId};

#[derive(PartialEq, Properties)]
pub struct IconButtonProps {
    pub icon_id: IconId,
    #[prop_or(None)]
    pub title: Option<AttrValue>,
    pub link: AttrValue,
    #[prop_or("2.5rem".into())]
    pub size: AttrValue,
    /// Icon color.
    #[prop_or(None)]
    pub color: Option<u32>,
    /// Colored circle background.
    #[prop_or(None)]
    pub circle_color: Option<u32>,
    #[prop_or(None)]
    pub style: Option<AttrValue>,
}

#[styled_component(IconButton)]
pub fn icon_button(props: &IconButtonProps) -> Html {
    let onclick = {
        let link = props.link.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_propagation();

            if let Err(e) = window().unwrap().open_with_url_and_target(&link, "_blank") {
                if cfg!(debug_assertions) {
                    js_hooks::console_log!("could not open link: {:?}", e);
                }
            }
        })
    };

    let class = css!(
        r#"
        color: white;
        cursor: pointer;
        user-select: none;
        vertical-align: bottom;
    "#
    );

    let mut style = props.circle_color.map(|color| format!("background-color: #{color:06x}; padding: 0.5rem; border-radius: 50%; overflow-clip-margin: padding-box;"));

    if let Some(color) = props.color {
        style
            .get_or_insert_default()
            .push_str(&format!("color: #{color:06x};"));
    }

    if let Some(extra) = props.style.as_ref() {
        style.get_or_insert_default().push_str(extra);
    }

    html! {
        <Icon
            icon_id={props.icon_id}
            title={props.title.clone()}
            {onclick}
            width={props.size.clone()}
            height={props.size.clone()}
            {style}
            {class}
        />
    }
}
