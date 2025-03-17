// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    markdown, translate, translated_text, use_features, use_game_constants, use_translator,
    EngineNexus, MarkdownOptions, NexusDialog, RouteLink, CONTACT_EMAIL,
};
use std::rc::Rc;
use stylist::yew::styled_component;
use yew::{html, AttrValue, Html};

#[styled_component(PrivacyDialog)]
pub fn privacy_dialog() -> Html {
    let t = use_translator();
    let game_constants = use_game_constants();
    let game_name = game_constants.name;
    let privacy_title = translate!(t, "{game_name} Privacy Policy");
    let features = use_features();
    let md = translated_text!(t, "privacy_md");

    let class = css!(
        r#"
        table {
            border-spacing: 0.5rem;
            text-align: left;
            width: 100%;
        }
    "#
    );

    html! {
        <NexusDialog close_enabled={true} title={privacy_title}>
            <div {class}>
                {privacy(&md, CONTACT_EMAIL, features.outbound.contact_info, features.chat, features.outbound.accounts.is_some(), features.cookie_consent || features.ad_privacy)}
            </div>
        </NexusDialog>
    }
}

pub fn privacy(
    md: &str,
    email: &str,
    contact_info: bool,
    chat: bool,
    accounts: bool,
    cookie_consent: bool,
) -> Html {
    let email: Html = email.to_owned().into();
    let components = Box::new(move |href: &str, content: &str| match href {
        "email" => Some(email.clone()),
        "contact_info" if contact_info => Some(Html::default()),
        "chat" if chat => Some(Html::default()),
        "accounts" if accounts => Some(Html::default()),
        "cookie_consent" if cookie_consent => Some(Html::default()),
        "/settings/" => Some(html! {
            <RouteLink<EngineNexus> route={EngineNexus::Settings}>{AttrValue::from(Rc::from(content))}</RouteLink<EngineNexus>>
        }),
        _ => None,
    });
    markdown(
        md,
        &MarkdownOptions {
            components,
            ..Default::default()
        },
    )
}
