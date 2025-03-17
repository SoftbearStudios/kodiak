// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_features, IconButton};
use yew::virtual_dom::AttrValue;
use yew::{function_component, html, Html, Properties};
use yew_icons::IconId;

#[derive(PartialEq, Properties)]
pub struct GooglePlayButtonProps {
    /// Google Play store repository link.
    pub google_play_url: AttrValue,
    #[prop_or("2.5rem".into())]
    pub size: AttrValue,
    #[prop_or(false)]
    pub circle: bool,
}

#[function_component(GooglePlayButton)]
pub fn google_play_button(props: &GooglePlayButtonProps) -> Html {
    let features = use_features();
    html! {
        if features.outbound.app_stores {
            <IconButton
                icon_id={IconId::ExtraGooglePlay}
                title={"Google Play"}
                link={props.google_play_url.clone()}
                size={props.size.clone()}
                circle_color={props.circle.then_some(0xffffff)}
                style={props.circle.then_some("padding-left: 0.65rem; padding-right: 0.35rem;")}
            />
        } else if !props.circle {
            {"Google Play"}
        }
    }
}
