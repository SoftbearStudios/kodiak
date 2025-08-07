// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    translate, use_client_request_callback, use_copy_invitation_link, use_core_state, use_ctw,
    use_invitation_request_callback, ArenaQuery, ClientRequest, EngineArenaSettings, GameClient,
    InvitationId, InvitationRequest, NexusDialog, ServerId, ServerKind,
};
use std::cmp::max;
use std::ops::Deref;
use std::str::FromStr;
use stylist::yew::styled_component;
use web_sys::HtmlInputElement;
use yew::{
    html, use_effect_with, use_state_eq, Callback, Html, InputEvent, MouseEvent, TargetCast,
};
use yew_router::hooks::use_navigator;
use yew_router::AnyRoute;

#[styled_component(PlayWithFriendsDialog)]
pub fn play_with_friends_dialog<G: GameClient>() -> Html {
    let ctw = use_ctw();
    let core_state = use_core_state();
    let temporaries_available = core_state.temporaries_available;
    let t = ctw.translator;
    let navigator = use_navigator().unwrap();

    let indented_block_style = css!(
        r#"
        display: table;
        margin: 0.25rem 0 0 1.5rem;
        "#
    );

    let option_style = css!(
        r#"
        padding-top: 1rem;
        text-align: left;
        "#
    );

    let span_invite_code_style = css!(
        r#"
        display: table-cell;
        padding-right: 1rem;
        text-decoration: underline;
        vertical-align: middle;
    "#
    );

    let vertical_align_style = css!(
        r#"
        display: table-cell;
        font-style: italic;
        padding-right: 0.25rem;
        vertical-align: middle;
    "#
    );

    #[derive(Copy, Clone, PartialEq)]
    enum Choice {
        AcceptInvitation,
        CreateInvitation,
        CreatePartyServer,
    }

    let bots = use_state_eq(|| {
        G::GAME_CONSTANTS
            .default_temporary_server_bots
            .max(G::GAME_CONSTANTS.min_temporary_server_bots)
            .min(G::GAME_CONSTANTS.max_temporary_server_bots)
    });
    let code = use_state_eq(String::new);

    let choice = use_state_eq(|| Choice::AcceptInvitation);
    let escape_party = {
        let set_server_id_callback = ctw.set_server_id_callback.clone();
        Callback::from(move |_: ()| {
            if ctw
                .setting_cache
                .arena_id
                .realm_id()
                .is_some_and(|r| r.is_temporary())
            {
                let server_id = ctw.setting_cache.server_id.unwrap();
                set_server_id_callback.emit((server_id, ArenaQuery::default()));
            }
        })
    };

    let choice_factory = |new: Choice| -> Option<Callback<InputEvent>> {
        let choice = choice.clone();
        let code = code.clone();
        if new == Choice::CreatePartyServer {
            let cb = ctw.set_server_id_callback.clone();
            ctw.setting_cache
                .server_id
                .filter(|_| temporaries_available)
                .map(move |server_id| {
                    cb.reform(move |_: InputEvent| {
                        code.set(String::new());
                        choice.set(new);
                        (server_id, ArenaQuery::NewTemporary)
                    })
                })
        } else {
            Some(escape_party.reform(move |_: InputEvent| {
                choice.set(new);
            }))
        }
    };

    let on_input_bots = {
        let bots = bots.clone();
        Callback::from(move |event: InputEvent| {
            let string = event.target_unchecked_into::<HtmlInputElement>().value();
            if let Ok(value) = u16::from_str(&string) {
                bots.set(value);
            }
        })
    };

    {
        let code = code.clone();
        use_effect_with(
            ctw.setting_cache.arena_id.invitation_id(),
            move |selected_invitation_id| {
                if let Some(invitation_id) = selected_invitation_id {
                    code.set(invitation_id.to_string());
                }
            },
        );
    }

    let invite_code_placeholder = translate!(t, "Invite Code");
    let invite_code_size = max(
        InvitationId::CODE_LEN as usize,
        invite_code_placeholder.len(),
    ) + 1;
    let oninput_code = {
        let code = code.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event.target_dyn_into::<HtmlInputElement>() {
                let mut value = input.value();

                if value.starts_with("http") {
                    value = value
                        .chars()
                        .rev()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .take(InvitationId::CODE_LEN as usize)
                        .collect();
                    value = value.chars().rev().collect();
                }

                let string = value
                    .chars()
                    .map(|c| c.to_ascii_uppercase())
                    .filter(|c| c.is_ascii_alphanumeric())
                    .take(InvitationId::CODE_LEN as usize)
                    .collect::<String>();
                code.set(string);
            }
        })
    };

    let party_invite = ctw
        .setting_cache
        .arena_id
        .realm_id()
        .and_then(|r| r.temporary());
    let onclick_copy_invite = use_copy_invitation_link(party_invite, false);
    let client_request_callback = use_client_request_callback();
    let invitation_request_callback = use_invitation_request_callback();
    let core_state = use_core_state();
    let onclick_ok = match *choice {
        Choice::AcceptInvitation => {
            let navigator = navigator.clone();
            let invitation_id = InvitationId::from_str(&*code).ok();
            invitation_id
                .map(|invitation_id| {
                    let invitation_server_id = ServerId {
                        number: invitation_id.server_number(),
                        kind: ctw
                            .setting_cache
                            .server_id
                            .map(|s| s.kind)
                            .unwrap_or(ServerKind::Cloud),
                    };
                    (invitation_server_id, invitation_id)
                })
                .filter(|&(invitation_server_id, _)| {
                    invitation_server_id.kind.is_local()
                        || ctw.setting_cache.server_id == Some(invitation_server_id)
                        || core_state
                            .servers
                            .iter()
                            .any(|((s, _), _)| *s == invitation_server_id)
                        || ctw.available_servers.contains(&invitation_server_id)
                })
                .map(|(invitation_server_id, invitation_id)| {
                    ctw.set_server_id_callback
                        .reform(move |_event: MouseEvent| {
                            navigator.push(&AnyRoute::new("/"));
                            (invitation_server_id, ArenaQuery::Invitation(invitation_id))
                        })
                })
        }
        Choice::CreateInvitation => {
            let invitation_request_callback = invitation_request_callback.clone();
            let navigator = navigator.clone();
            let created_invitation_id = core_state.created_invitation_id;
            created_invitation_id.map(|created_invitation_id| {
                // We could maintain this state on the client, but informing the server
                // is better for metrics/quests.
                invitation_request_callback.reform(move |_event: MouseEvent| {
                    navigator.push(&AnyRoute::new("/"));
                    InvitationRequest::Accept(Some(created_invitation_id))
                })
            })
        }
        Choice::CreatePartyServer => {
            let navigator = navigator.clone();
            let bots = bots.clone();
            Some(client_request_callback.reform(move |_: MouseEvent| {
                navigator.push(&AnyRoute::new("/"));
                ClientRequest::ArenaSettings(
                    serde_json::to_string(&EngineArenaSettings {
                        bots: Some(*bots),
                        bot_aggression: None,
                    })
                    .unwrap(),
                )
            }))
        }
    };

    let onclick_cancel = match *choice {
        Choice::AcceptInvitation => {
            let code = code.clone();
            let navigator = navigator.clone();
            Callback::from(move |_event: MouseEvent| {
                code.set(String::new());
                navigator.push(&AnyRoute::new("/"));
            })
        }
        Choice::CreateInvitation => {
            let navigator = navigator.clone();
            Callback::from(move |_: MouseEvent| {
                navigator.push(&AnyRoute::new("/"));
            })
        }
        Choice::CreatePartyServer => {
            let navigator = navigator.clone();
            escape_party.reform(move |_: MouseEvent| {
                navigator.push(&AnyRoute::new("/"));
            })
        }
    };

    html! {
        <NexusDialog
            ok_enabled={true}
            {onclick_cancel}
            {onclick_ok}
            responsive={true}
            title={translate!(t, "Play with friends")}
            >
            <div id="choice_panel">
                <div class={option_style.clone()}>
                    <input
                        id="accept"
                        checked={*choice == Choice::AcceptInvitation}
                        oninput={choice_factory(Choice::AcceptInvitation)}
                        name="pwf_choice"
                        type="radio"
                        value="accept_invitation"
                    />
                    <label for="accept">{translate!(t, "Accept an invitation from a friend")}</label>
                </div>
                if *choice == Choice::AcceptInvitation {
                    <div class={indented_block_style.clone()}>
                        <input
                            autocapitalize="off"
                            autocomplete="off"
                            autocorrect="off"
                            autocapitalize="off"
                            disabled={*choice != Choice::AcceptInvitation}
                            oninput={oninput_code}
                            placeholder={invite_code_placeholder}
                            size={format!("{invite_code_size}")}
                            spellcheck="false"
                            type="text"
                            value={code.deref().clone()}
                        />
                    </div>
                }
                <div class={option_style.clone()}>
                    <input
                        id="public"
                        checked={*choice == Choice::CreateInvitation}
                        oninput={choice_factory(Choice::CreateInvitation)}
                        name="pwf_choice"
                        type="radio"
                        value="public_server"
                    />
                    <label for="public">{translate!(t, "Invite friends to play with you on the public server")}</label>
                </div>
                if *choice == Choice::CreateInvitation {
                    <div class={indented_block_style.clone()}>
                        <span class={vertical_align_style.clone()}>{translate!(t, "Invite code")}{":"}</span>
                        <span
                            class={span_invite_code_style.clone()}
                            title={translate!(t, "Invite players to this server without them joining an existing team")}
                        >{core_state.created_invitation_id.filter(|_| *choice == Choice::CreateInvitation && ctw.setting_cache.arena_id.realm_id().map(|r| r.is_public_default()).unwrap_or(true)).map(|i| i.to_string()).unwrap_or_default()}</span>
                        <button
                            disabled={*choice != Choice::CreateInvitation}
                            onclick={onclick_copy_invite.clone()}
                        >{translate!(t, "Copy to clipboard")}</button>
                    </div>
                }
                <div class={option_style}>
                    <input
                        id="party"
                        checked={*choice == Choice::CreatePartyServer}
                        oninput={choice_factory(Choice::CreatePartyServer)}
                        name="pwf_choice"
                        type="radio"
                        value="party_server"
                    />
                    <label for="party">{translate!(t, "Invite friends to play with you on a party server")}</label>
                </div>
                if *choice == Choice::CreatePartyServer {
                    if temporaries_available || ctw.setting_cache.arena_id.realm_id().is_some_and(|r| r.is_temporary()) {
                        <div class={indented_block_style.clone()}>
                            <span class={vertical_align_style.clone()}>{translate!(t, "Invite code:")}</span>
                            <span class={span_invite_code_style.clone()}>{party_invite.map(|p| p.to_string()).unwrap_or_default()}</span>
                            <button
                                disabled={*choice != Choice::CreatePartyServer}
                                onclick={onclick_copy_invite}
                            >{translate!(t, "Copy to clipboard")}</button>
                        </div>
                        <div class={indented_block_style.clone()}>
                            <span class={vertical_align_style.clone()}>
                                {translate!(t, "Bots")}{": "}{*bots}
                            </span>
                            <input
                                class={vertical_align_style.clone()}
                                max={G::GAME_CONSTANTS.max_temporary_server_bots.to_string()}
                                min={G::GAME_CONSTANTS.min_temporary_server_bots.to_string()}
                                oninput={on_input_bots}
                                step="1"
                                type="range"
                                value={bots.to_string()}
                            />
                        </div>
                    } else {
                        <div class={indented_block_style.clone()}>
                            {translate!(t, "Party server limit reached. Try again later.")}
                        </div>
                    }
                }
            </div>
        </NexusDialog>
    }
}
