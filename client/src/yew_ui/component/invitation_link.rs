// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_core_state, use_ctw, use_translator, InvitationId, InvitationLinks};
use gloo::timers::callback::Timeout;
use stylist::yew::styled_component;
use web_sys::{window, MouseEvent};
use yew::{hook, html, use_state, AttrValue, Callback, Children, Html, Properties};

#[derive(PartialEq, Properties)]
pub struct InvitationLinkProps {
    #[prop_or(None)]
    pub override_id: Option<InvitationId>,
    #[prop_or(false)]
    pub show_code: bool,
    #[prop_or_default]
    pub children: Children,
    #[prop_or(None)]
    pub style: Option<AttrValue>,
}

#[styled_component(InvitationLink)]
pub fn invitation_link(props: &InvitationLinkProps) -> Html {
    let t = use_translator();
    let created_invitation_id = props.override_id.or(use_core_state().created_invitation_id);
    let onclick = use_copy_invitation_link(props.override_id, false);

    let mut style = String::from("color: white;user-drag: none;-webkit-user-drag: none;");

    let (contents, opacity) = if onclick.is_some() {
        let contents = if !props.children.is_empty() {
            html! {<>{props.children.clone()}</>}
        } else if let Some(created_invitation_id) = created_invitation_id
            && props.show_code
        {
            html! {{AttrValue::from(format!("Invite: {created_invitation_id}"))}}
        } else {
            html! {{AttrValue::from(t.invitation_label())}}
        };
        (contents, "opacity: 1.0;cursor: pointer;")
    } else {
        (
            html! {{AttrValue::from(t.invitation_copied_label())}},
            "opacity: 0.6;cursor: default;text-decoration:none;",
        )
    };

    if let Some(style_override) = &props.style {
        style += style_override;
    }
    /*
    else if props.show_code {
        style += "text-decoration: none; font-size: 1.5rem;";
    }
    */
    style += opacity;

    // Trick yew into not warning about bad practice.
    let href: &'static str = "javascript:void(0)";

    html! {
        <a
            {href}
            {onclick}
            {style}
            title={AttrValue::from(t.invitation_label().to_owned())}
        >
            {contents}
        </a>
    }
}

/// [`None`] indicates the button was pressed recently.
#[hook]
pub fn use_copy_invitation_link(
    override_id: Option<InvitationId>,
    code_only: bool,
) -> Option<Callback<MouseEvent>> {
    let ctw = use_ctw();
    let invitation_links = ctw.features.outbound.invitations;
    let timeout = use_state::<Option<Timeout>, _>(|| None);
    let created_invitation_id = override_id.or(use_core_state().created_invitation_id);

    timeout.is_none().then(|| {
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_propagation();

            let window = window().unwrap();

            if let Some(invitation_id) = created_invitation_id {
                let invitation_link = if !code_only
                    && !invitation_links.is_none()
                    && let Some(origin) = window.location().origin().ok()
                {
                    match &invitation_links {
                        InvitationLinks::Template(template) => {
                            template.replace("GAME_WILL_REPLACE", &invitation_id.to_string())
                        }
                        InvitationLinks::Verbatim(verbatim) => verbatim.to_string(),
                        _ => {
                            format!("{}/invite/{}/", origin, invitation_id)
                        }
                    }
                } else {
                    invitation_id.to_string()
                };

                let clipboard = window.navigator().clipboard();
                // TODO: await this.
                let _ = clipboard.write_text(&invitation_link);

                let timeout_clone = timeout.clone();

                timeout.set(Some(Timeout::new(2000, move || {
                    timeout_clone.set(None);
                })));
            }
        })
    })
}
