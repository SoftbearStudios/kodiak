// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_features, IconButton};
use yew::virtual_dom::AttrValue;
use yew::{function_component, html, Html, Properties};
use yew_icons::IconId;

#[derive(PartialEq, Properties)]
pub struct DiscordButtonProps {
    /// Discord invite link (defaults to Softbear discord server).
    #[prop_or("https://discord.gg/YMheuFQWTX".into())]
    pub invite_link: AttrValue,
    #[prop_or("2.5rem".into())]
    pub size: AttrValue,
    #[prop_or(false)]
    pub circle: bool,
}

#[function_component(DiscordButton)]
pub fn discord_button(props: &DiscordButtonProps) -> Html {
    let features = use_features();
    html! {
        if features.outbound.social_media {
            <IconButton
                icon_id={IconId::BootstrapDiscord}
                title={"Discord"}
                link={props.invite_link.clone()}
                size={props.size.clone()}
                color={props.circle.then_some(0x5865F2)}
                circle_color={props.circle.then_some(0xffffff)}
            />
        } else if !props.circle {
            {"Discord"}
        }
    }
}
