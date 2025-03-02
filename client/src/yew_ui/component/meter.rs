// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use stylist::yew::styled_component;
use yew::{classes, html, Children, Classes, Html, Properties};

#[derive(PartialEq, Properties)]
pub struct MeterProps {
    pub children: Children,
    #[prop_or(0x0084b1)]
    pub color: u32,
    #[prop_or(0xbbbbbb)]
    pub background_color: u32,
    #[prop_or(1)]
    pub border_width: u8,
    #[prop_or(0xffffff)]
    pub border_color: u32,
    #[prop_or_default]
    pub class: Classes,
    /// 0 to 1.
    pub value: f32,
}

#[styled_component(Meter)]
pub fn meter(props: &MeterProps) -> Html {
    let div_css_class = css!(
        r#"
        border-style: solid;
        border-radius: 0.5rem;
        box-sizing: border-box;
        color: white;
        font-weight: bold;
        height: min-content;
        min-height: 1.1rem;
        overflow: hidden;
        padding: 0.2rem;
        text-align: center;
        transition: background-size 0.5s;
        user-select: none;
        width: 100%;
    "#
    );

    let percentage = (props.value.clamp(0.0, 1.0) * 100.0).round();
    let background_size = (percentage.max(1.0) * 100.0).round();

    let style = format!("background: linear-gradient(90deg, #{:06x} 0%, #{:06x} 1%, #{:06x} 1%, #{:06x} 100%); background-origin: border-box; background-size: {}%; border-width: {}px; border-color: #{:06x};", props.color, props.color, props.background_color, props.background_color, background_size, props.border_width, props.border_color);

    html! {
        <div class={classes!(div_css_class, props.class.clone())} {style}>
            {props.children.clone()}
        </div>
    }
}
