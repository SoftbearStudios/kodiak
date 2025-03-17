// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_copy_invitation_link, use_translator};
use yew::virtual_dom::AttrValue;
use yew::{function_component, html, Html, Properties};
use yew_icons::{Icon, IconId};

#[derive(PartialEq, Properties)]
pub struct InvitationButtonProps {
    #[prop_or("2rem".into())]
    pub size: AttrValue,
}

#[function_component(InvitationButton)]
pub fn invitation_button(props: &InvitationButtonProps) -> Html {
    let t = use_translator();
    let onclick = use_copy_invitation_link(None, false);
    let (title, style) = if onclick.is_some() {
        (t.invitation_label(), "opacity: 1.0; cursor: pointer;")
    } else {
        (
            t.invitation_copied_label(),
            "opacity: 0.6; cursor: default;",
        )
    };
    html! {
        <Icon icon_id={IconId::BootstrapPersonPlus} {title} {onclick} width={props.size.clone()} height={props.size.clone()} style={format!("color: white; cursor: pointer; user-select: none; vertical-align: bottom; {}", style)}/>
    }
}
