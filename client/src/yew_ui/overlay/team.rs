// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    event_target, high_contrast_class, use_change_common_settings_callback, use_core_state,
    use_ctw, use_translator, InvitationLink, JoinedStatus, Manifestation, Member, PlayerDto,
    PlayerId, Position, Positioner, Team, TeamId, TeamName, TeamRequest, TranslateFn,
};
use kodiak_common::arrayvec::ArrayVec;
use std::rc::Rc;
use stylist::yew::styled_component;
use web_sys::HtmlInputElement;
use yew::{
    classes, html, html_nested, use_node_ref, use_state_eq, AttrValue, Callback, Html, InputEvent,
    MouseEvent, Properties, SubmitEvent,
};

pub fn make_team_dtos<'a, D: 'a, M: Manifestation + 'a>(
    teams: impl Iterator<Item = (TeamId, &'a Team<D, M>)>,
) -> Vec<TeamDto> {
    let mut ret = teams
        .filter_map(|(team_id, team)| {
            team.name
                .map(|name| TeamDto {
                    team_id,
                    name,
                    full: team.members.len() >= M::MAX_MEMBERS,
                    closed: (team.members.len() + team.joiners.len()) >= M::MAX_MEMBERS_AND_JOINERS,
                })
                .filter(|_| {
                    team.iter()
                        .next()
                        .map(|m| m.player_id.is_client())
                        .unwrap_or(false)
                })
        })
        .collect::<Vec<_>>();
    ret.sort();
    ret.truncate(5);
    ret
}

#[derive(PartialEq, Properties)]
pub struct TeamOverlayProps<D: PartialEq, M: PartialEq> {
    pub position: Position,
    pub team: Team<D, M>,
    pub joins: ArrayVec<TeamId, { JoinedStatus::MAX_JOINS }>,
    pub teams: Vec<TeamDto>,
    /// Override the default placeholder.
    #[prop_or(None)]
    pub name_placeholder: Option<TranslateFn>,
    pub team_request_callback: Callback<TeamRequest>,
}

/// The Team Data Transfer Object (DTO) binds team ID to team name.
// Field order matters for sorting.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TeamDto {
    /// Maximum number of numbers reached.
    pub full: bool,
    /// Closed to additional requests.
    pub closed: bool,
    pub name: TeamName,
    pub team_id: TeamId,
}

#[allow(unused)]
#[styled_component(TeamOverlay)]
pub fn team_overlay<
    D: PartialEq + std::fmt::Debug,
    M: PartialEq + Manifestation + std::fmt::Debug,
>(
    props: &TeamOverlayProps<D, M>,
) -> Html {
    let button_css_class = css!(
        r#"
        background-color: transparent;
        border: 0;
        border-radius: 0.25rem;
        box-sizing: border-box;
        color: white;
        cursor: pointer;
        font-size: 1rem;
        opacity: 0.9;
        padding: 0.1rem 0.5rem;
        white-space: nowrap;
        width: min-content;

        :disabled {
            opacity: 0.6;
            cursor: initial;
        }

        :hover:not(:disabled) {
            opacity: 1.0;
        }
        "#
    );

    let hidden_css_class = css!(
        r#"
        visibility: hidden;
        "#
    );

    let disabled_css_class = css!(
        r#"
        opacity: 0.6;
        cursor: initial;
        "#
    );

    let input_css_class = css!(
        r#"
        background-color: #00000025;
        border: 0;
        border-radius: 0.25rem;
        box-sizing: border-box;
        color: white;
        cursor: pointer;
        font-size: 1rem;
        font-weight: bold;
        margin-left: -0.25rem;
        outline: 0;
        pointer-events: all;
        white-space: nowrap;
        width: 6rem;

        @media (max-width: 600px) {
            width: 5rem;
        }
        "#
    );

    let invitation_link_style = r#"
        cursor: pointer;
        font-weight: bold;
        "#;

    let table_css_class = css!(
        r#"
        border-spacing: 0;
        color: white;
        width: 100%;
        "#
    );

    let tr_css_class = css!(
        r#"
        margin-top: 0.25rem;
        margin-bottom: 0.25rem;
        "#
    );

    let name_css_class = css!(
        r#"
        color: white;
        font-weight: bold;
        opacity: 0.9;
        white-space: nowrap;
    "#
    );

    let name_pending_css_class = css!(
        r#"
        filter: brightness(0.7);
    "#
    );

    let owner_css_class = css!(
        r#"
        text-decoration: none;
    "#
    );

    let underline_css_class = css!(
        r#"
        text-decoration: underline;
        "#
    );

    let ctw = use_ctw();
    let core_state = use_core_state();
    let t = use_translator();
    let high_contrast_class = high_contrast_class!(ctw, css);
    let team_request_callback = &props.team_request_callback;
    let input_ref = use_node_ref();
    let change_common_settings_callback = use_change_common_settings_callback();

    let team_name_empty = use_state_eq(|| true);
    // Can't auto-spawn..
    //let invitation_id = use_state_eq(|| Option::<InvitationId>::None);

    let on_new_team_name_change = {
        let team_name_empty = team_name_empty.clone();
        //let invitation_id = invitation_id.clone();
        move |event: InputEvent| {
            if !event.is_composing() {
                let input: HtmlInputElement = event_target(&event);
                let value = input.value();
                team_name_empty.set(value.is_empty());
                //invitation_id.set(InvitationId::from_str(&value).ok())
            }
        }
    };

    let on_accept_join_team = {
        let cb = team_request_callback.clone();
        move |player_id: PlayerId| {
            cb.emit(TeamRequest::Accept(player_id));
        }
    };

    let on_create_team = {
        let cb = team_request_callback.clone();
        let change_common_settings_callback = change_common_settings_callback.clone();
        let input_ref = input_ref.clone();
        move || {
            if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                let new_team_name = input.value();
                if !new_team_name.is_empty() {
                    /*
                    if let Ok(invitation_id) = InvitationId::from_str(&new_team_name) {
                        change_common_settings_callback.emit(Box::new(
                            move |common_settings, browser_storages| {
                                // Hack: make invites work on same server by flushing the token.
                                common_settings
                                    .set_token(None, browser_storages);
                                common_settings
                                    .set_invitation_id(Some(invitation_id), browser_storages);
                            },
                        ));
                    } else {
                    */
                    cb.emit(TeamRequest::Name(TeamName::new_input_sanitized(
                        &new_team_name,
                    )));
                }
            }
        }
    };
    let on_create_team_2 = on_create_team.clone();

    let on_kick_from_team = {
        let cb = team_request_callback.clone();
        move |player_id: PlayerId| {
            cb.emit(TeamRequest::Kick(player_id));
        }
    };

    let on_leave_team = {
        let cb = team_request_callback.clone();
        move || cb.emit(TeamRequest::Leave)
    };

    let on_reject_join_team = {
        let cb = team_request_callback.clone();
        move |player_id: PlayerId| {
            cb.emit(TeamRequest::Reject(player_id));
        }
    };

    let on_request_join_team = {
        let cb = team_request_callback.clone();
        move |team_id: TeamId| {
            cb.emit(TeamRequest::Join(team_id));
        }
    };

    const CHECK_MARK: &str = "✔";
    const X_MARK: &str = "✘";

    let my_player_id = use_core_state().player_id;
    let i_am_team_captain = my_player_id == props.team.leader().map(|l| l.player_id); // TODO
    let team_full = props.team.members.len() > M::MAX_MEMBERS;
    #[cfg(feature = "pointer_lock")]
    let hide_buttons = ctw.pointer_locked;
    #[cfg(not(feature = "pointer_lock"))]
    let hide_buttons = false;

    html! {
        <Positioner
            id="team"
            position={props.position}
            class={classes!(high_contrast_class)}
        >
            if let Some(team_name) = &props.team.name {
                <table class={table_css_class}>
                    <tr>
                        <th colspan="3">
                            <InvitationLink show_code={true} style={invitation_link_style}>{AttrValue::from(Rc::from(team_name.as_str()))}</InvitationLink>
                        </th>
                    </tr>
                    {props.team.iter().enumerate().filter_map(|(i, Member{player_id, ..})| core_state.player_or_bot(*player_id).map(|p| (i, p))).map(|(i, PlayerDto{alias, player_id, ..})| {
                        let me = my_player_id == Some(player_id);
                        let team_captain = i == 0;
                        let on_leave_team = on_leave_team.clone();
                        let on_kick_from_team = on_kick_from_team.clone();

                        html_nested!{
                            <tr class={tr_css_class.clone()}>
                                <td class={classes!(
                                    name_css_class.clone(),
                                    team_captain.then(|| owner_css_class.clone()))
                                }>{alias.as_str()}</td>
                                if !hide_buttons {
                                    // for spacing only.
                                    <td><button
                                        class={classes!(button_css_class.clone(), hidden_css_class.clone())}
                                    >{CHECK_MARK}</button></td>
                                    <td><button
                                        class={classes!(
                                            button_css_class.clone(),
                                            (
                                                (!me && !i_am_team_captain)
                                                || (me && !M::MEMBERS_CAN_LEAVE)
                                                || (team_captain && !M::LEADER_CAN_LEAVE)
                                                || (props.team.members.len() == 1 && !M::CAN_LEAVE_SOLO_TEAM)
                                            ).then(|| hidden_css_class.clone()))
                                        }
                                        onclick={move |event: MouseEvent| {
                                            event.stop_propagation();
                                            if me {
                                                on_leave_team();
                                            } else {
                                                on_kick_from_team(player_id);
                                            }
                                        }}
                                        title={if me {
                                            t.team_leave_hint()
                                        } else {
                                            t.team_kick_hint()
                                        }}
                                    >{X_MARK}</button></td>
                                }
                            </tr>
                        }
                    }).collect::<Html>()}
                    if i_am_team_captain {
                        {props.team.joiners.iter().filter_map(|player_id| core_state.player_or_bot(*player_id)).map(|PlayerDto{alias, player_id, ..}| {
                            let on_accept_join_team = on_accept_join_team.clone();
                            let on_reject_join_team = on_reject_join_team.clone();
                            html_nested!{
                                <tr class={tr_css_class.clone()}>
                                    <td class={classes!(name_css_class.clone(), name_pending_css_class.clone())}>{alias.as_str()}</td>
                                    if !hide_buttons {
                                        <td><button
                                            class={classes!(button_css_class.clone(), team_full.then(|| disabled_css_class.clone()))}
                                            onclick={move |event: MouseEvent| {
                                                event.stop_propagation();
                                                on_accept_join_team(player_id);
                                            }}
                                            title={t.team_accept_hint()}
                                        >{CHECK_MARK}</button></td>
                                        <td><button
                                            class={button_css_class.clone()}
                                            onclick={move |event: MouseEvent| {
                                                event.stop_propagation();
                                                on_reject_join_team(player_id);
                                            }}
                                            title={t.team_deny_hint()}
                                        >{X_MARK}</button></td>
                                    }
                                </tr>
                            }
                        }).collect::<Html>()}
                    }
                </table>
            } else if !hide_buttons {
                <form onsubmit={move |e: SubmitEvent| {
                    e.prevent_default();
                    e.stop_propagation();
                    on_create_team();
                }}>
                    <table class={table_css_class}>
                        <tr>
                            <td>
                                <input
                                    ref={input_ref}
                                    type="text"
                                    minlength="1"
                                    maxlength="6"
                                    placeholder={if let Some(placeholder) = &props.name_placeholder {
                                        (placeholder)(&t)
                                    } else {
                                        t.team_name_placeholder()
                                    }}
                                    oninput={on_new_team_name_change}
                                    class={input_css_class}
                                />
                            </td>
                            <td>
                                <button
                                    disabled={*team_name_empty}
                                    class={classes!(button_css_class.clone(), underline_css_class.clone(), team_name_empty.then(|| hidden_css_class.clone()))}
                                    onclick={move |event: MouseEvent| {
                                        event.prevent_default();
                                        event.stop_propagation();
                                        on_create_team_2();
                                    }}
                                >
                                    /*
                                    if invitation_id.is_some() {
                                        {t.team_accept_hint()}
                                    } else {
                                    */
                                    {t.team_create_hint()}
                                </button>
                            </td>
                        </tr>
                        {props.teams.iter().map(|&TeamDto{closed, name, team_id, ..}| {
                            let on_request_join_team = on_request_join_team.clone();
                            let unavailable = closed || props.joins.contains(&team_id);

                            html_nested!{
                                <tr>
                                    <td class={name_css_class.clone()}>{name.as_str()}</td>
                                    <td>
                                        <button
                                            type="button"
                                            class={classes!(
                                                button_css_class.clone(),
                                                underline_css_class.clone(),
                                                unavailable.then(|| hidden_css_class.clone())
                                            )}
                                            onclick={move |event: MouseEvent| {
                                                event.stop_propagation();
                                                on_request_join_team(team_id);
                                            }}
                                        >
                                            {t.team_request_hint()}
                                        </button>
                                    </td>
                                </tr>
                            }
                        }).collect::<Html>()}
                    </table>
                </form>
            }
        </Positioner>
    }
}
