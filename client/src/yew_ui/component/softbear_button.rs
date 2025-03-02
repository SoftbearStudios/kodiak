// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::js_hooks::{self, window};
use crate::use_features;
use stylist::yew::styled_component;
use yew::virtual_dom::AttrValue;
use yew::{html, Callback, Html, MouseEvent, Properties};

#[derive(PartialEq, Properties)]
pub struct SoftbearButtonProps {
    #[prop_or("2.5rem".into())]
    pub size: AttrValue,
}

#[styled_component(SoftbearButton)]
pub fn softbear_button(props: &SoftbearButtonProps) -> Html {
    let features = use_features();
    let class = css!(
        r#"
        color: #6e00b3;
        background-color: #ffffff;
        padding: 0.5rem;
        border-radius: 50%;
        overflow-clip-margin: padding-box;
        cursor: pointer;
        user-select: none;
        vertical-align: bottom;
    "#
    );
    if !features.outbound.promo {
        return Default::default();
    }
    let onclick = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        e.stop_propagation();

        if let Err(e) = window().open_with_url_and_target("https://softbear.com", "_blank") {
            if cfg!(debug_assertions) {
                js_hooks::console_log!("could not open link: {:?}", e);
            }
        }
    });
    // https://www.fiverr.com/vintage_valley/design-professional-business-logo
    html! {
        <svg
            version="1.1"
            xmlns="http://www.w3.org/2000/svg"
            x="0px"
            y="0px"
            viewBox="0 0 318.68 275"
            //viewBox="0 0 318.68 285.39"
            //style="enable-background:new 0 0 318.68 285.39;"
            width={props.size.clone()}
            height={props.size.clone()}
            fill="currentColor"
            {onclick}
            {class}
        >
            <title>{"Softbear"}</title>
            <path class="st0" d="M300.19,88.74c19.85-25.68,18.66-59.87-2.68-76.37c-21.34-16.51-54.74-9.08-74.59,16.61
                c-0.84,1.08-1.62,2.18-2.39,3.28c-18.66-7.4-39.36-11.54-61.18-11.54s-42.53,4.13-61.18,11.54c-0.77-1.1-1.55-2.21-2.39-3.28
                C75.91,3.29,42.51-4.14,21.17,12.36c-21.34,16.51-22.54,50.7-2.68,76.37c1.97,2.55,4.08,4.92,6.3,7.1
                c-8.93,16.74-13.94,35.42-13.94,55.12c0,71.92,66.49,130.24,148.5,130.24s148.5-58.31,148.5-130.24
                c0-19.71-5.01-38.38-13.94-55.12C296.11,93.66,298.23,91.29,300.19,88.74z M35.31,62.82c-2.14-4.86-3.22-9.94-3.04-14.72
                c0.12-3.47,1.06-9.86,5.98-13.66c2.53-1.95,5.71-2.95,9.48-2.95c5.73,0,11.81,2.22,17.25,6.13C53.9,44.84,43.92,53.32,35.31,62.82
                z M92.41,93.62c1.32-4.43,5.59-7.06,9.57-5.87c3.96,1.18,6.1,5.71,4.78,10.14c-1.32,4.43-5.59,7.06-9.56,5.87
                C93.24,102.57,91.1,98.03,92.41,93.62z M159.34,213.65c-78.62,0-58.49-74.29-58.49-74.29s13.74-65.21,58.49-65.21
                s58.49,65.21,58.49,65.21S237.96,213.65,159.34,213.65z M221.47,103.76c-3.96,1.18-8.24-1.45-9.56-5.87s0.82-8.96,4.78-10.14
                c3.97-1.18,8.25,1.45,9.57,5.87C227.58,98.03,225.44,102.57,221.47,103.76z M283.37,62.82c-8.61-9.5-18.59-17.98-29.66-25.21
                c5.44-3.91,11.52-6.13,17.25-6.13c3.76,0,6.95,1,9.48,2.95c4.92,3.8,5.86,10.19,5.98,13.66
                C286.58,52.88,285.51,57.96,283.37,62.82z"/>
            <path class="st0" d="M174.7,152.08c-6.07-0.03-8.98-2.75-10.37-5.24c-0.89-1.58-1.27-3.39-1.27-5.2v-19.88
                c0-4.88,3.96-8.83,8.83-8.83h4.03c5.89,0,10.13-5.66,8.47-11.32c-2.05-6.98-8.15-14.62-25.06-14.62s-23.01,7.63-25.06,14.62
                c-1.66,5.66,2.58,11.32,8.47,11.32h4.03c4.88,0,8.83,3.96,8.83,8.83v19.88c0,1.81-0.39,3.62-1.27,5.2
                c-1.39,2.49-4.3,5.21-10.37,5.24c-11.21,0.06-20.02-4.41-19.51,2.83c0.5,7.24,16.24,7.68,23.48,4.85
                c7.24-2.83,7.44-3.02,11.4-3.02c3.96,0,4.16,0.19,11.4,3.02c7.24,2.83,22.98,2.39,23.48-4.85
                C194.72,147.67,185.91,152.14,174.7,152.08z"/>
        </svg>
    }
}
