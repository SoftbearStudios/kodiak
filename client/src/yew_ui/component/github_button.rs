// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_features, IconButton};
use yew::virtual_dom::AttrValue;
use yew::{function_component, html, Html, Properties};
use yew_icons::IconId;

#[derive(PartialEq, Properties)]
pub struct GithubButtonProps {
    /// Github repository link.
    pub repository_url: AttrValue,
    #[prop_or("2.5rem".into())]
    pub size: AttrValue,
    #[prop_or(false)]
    pub circle: bool,
}

#[function_component(GithubButton)]
pub fn github_button(props: &GithubButtonProps) -> Html {
    let features = use_features();
    html! {
        if features.outbound.social_media {
            <IconButton
                icon_id={IconId::BootstrapGithub}
                title={"GitHub"}
                link={props.repository_url.clone()}
                size={props.size.clone()}
                color={props.circle.then_some(0x000000)}
                circle_color={props.circle.then_some(0xffffff)}
            />
        } else if !props.circle {
            {"GitHub"}
        }
    }
}
