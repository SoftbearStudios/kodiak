// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    markdown, translate, translated_text, use_features, use_game_constants, use_translator,
    EngineNexus, MarkdownOptions, NexusDialog, RouteLink, CONTACT_EMAIL,
};
use std::rc::Rc;
use yew::{function_component, html, AttrValue, Html};

#[function_component(TermsDialog)]
pub fn terms_dialog() -> Html {
    let t = use_translator();
    let game_constants = use_game_constants();
    let game_name = game_constants.name;
    let terms_title = translate!(t, "{game_name} Terms of Service");
    let features = use_features();
    let md = translated_text!(t, "terms_md");

    html! {
        <NexusDialog close_enabled={true} title={terms_title}>
            {terms(&md, game_constants.name, "game", &[game_constants.trademark], features.outbound.contact_info, features.chat, features.outbound.accounts.is_some())}
        </NexusDialog>
    }
}

/// noun may be "game" or "games"
pub fn terms(
    md: &str,
    site: &str,
    noun: &str,
    trademarks: &[&'static str],
    contact_info: bool,
    chat: bool,
    accounts: bool,
) -> Html {
    let site: Html = site.to_owned().into();
    let noun: Html = noun.to_owned().into();
    let trademarks = html! {
        {trademarks.iter().map(|&trademark| html!{
            <b>{trademark}</b>
        }).intersperse(html!{", "}).collect::<Html>()}
    };
    let components = Box::new(move |href: &str, content: &str| match href {
        "/privacy/" => Some(html! {
            <RouteLink<EngineNexus> route={EngineNexus::Privacy}>{AttrValue::from(Rc::from(content))}</RouteLink<EngineNexus>>
        }),
        "email" => Some(CONTACT_EMAIL.to_owned().into()),
        "noun" => Some(noun.clone()),
        "site" => Some(site.clone()),
        "trademarks" => Some(trademarks.clone()),
        "contact_info" if contact_info => Some(Html::default()),
        "chat" if chat => Some(Html::default()),
        "accounts" if accounts => Some(Html::default()),
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
