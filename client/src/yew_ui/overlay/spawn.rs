// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    translate, use_banner_ad, use_change_common_settings_callback, use_core_state, use_ctw,
    use_features, use_interstitial_ad, use_invitation_request_callback, use_navigation,
    use_translator, ArenaQuery, BannerAd, Ctw, EngineNexus, GlobalEventListener, InterstitialAd,
    InvitationRequest, PlayerAlias, Position, Positioner,
};
use gloo::timers::callback::Timeout;
use stylist::yew::styled_component;
use web_sys::{HtmlInputElement, MessageEvent};
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct SpawnOverlayProps {
    // Override nexus.
    //pub spawning: bool,
    pub on_play: Callback<PlayerAlias>,
    #[prop_or(Position::Center)]
    pub position: Position,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub input_style: AttrValue,
    #[prop_or("border: 2px solid #8dd391;".into())]
    pub button_style: AttrValue,
    #[prop_or(true)]
    pub animation: bool,
    /// Move promo container to the right side of the spawn overlay. Otherwise,
    /// it is on the left side of the screen.
    #[prop_or(false)]
    pub promo_right_side: bool,
    #[prop_or(false)]
    pub beta: bool,
}

#[styled_component(SpawnOverlay)]
pub fn spawn_overlay(props: &SpawnOverlayProps) -> Html {
    let form_style = css!(
        r#"
        display: flex;
        flex-direction: column;
        position: absolute;
        row-gap: 2rem;
        user-select: none;
        min-width: 50%;

        @keyframes fadein {
            from { opacity: 0; }
            to   { opacity: 1; }
        }
    "#
    );

    let animation_style = css!(
        r#"
        animation: fadein 1s;
    "#
    );

    let input_style = css!(
        r#"
        background-color: #00000025;
        border-radius: 3rem;
        border: 0;
        box-sizing: border-box;
        color: #FFFA;
        cursor: pointer;
        font-size: 1.7rem;
        font-weight: bold;
        margin-top: 0.25em;
        outline: 0;
        padding: 1.2rem;
        pointer-events: all;
        text-align: center;
        white-space: nowrap;
        width: 100%;
   "#
    );

    let button_style = css!(
        r#"
        background-color: #549f57;
        border-radius: 1rem;
        border: 1px solid #61b365;
        box-sizing: border-box;
        color: white;
        cursor: pointer;
        font-size: 3.25rem;
        padding-bottom: 0.7rem;
        padding-top: 0.5rem;
        text-decoration: none;
        white-space: nowrap;
        width: 100%;

        :disabled {
            filter: brightness(0.8);
            cursor: initial;
        }

        :hover:not(:disabled) {
            filter: brightness(0.95);
        }

        :active:not(:disabled) {
            filter: brightness(0.9);
        }
    "#
    );

    let t = use_translator();
    let features = use_features();
    let (paused, transitioning, onanimationend) = use_splash_screen();
    if !props.animation
        && let Some(onanimationend) = onanimationend.as_ref()
    {
        onanimationend.emit(());
    }
    let ctw = use_ctw();

    // Replaced by set_ui_props
    /*
    {
        let set_escaping_callback = ctw.set_escaping_callback.clone();
        use_effect_with(
            ctw.escaping,
            move |&escaping| {
                if escaping.is_in_game() || escaping.is_escaping() {
                    set_escaping_callback.emit(Escaping::Spawning);
                }
            },
        );
    }
    */

    let previous_alias = ctw.setting_cache.alias;
    let random_guest_alias = ctw.setting_cache.random_guest_alias;
    let input_ref = use_node_ref();
    let interstitial_ad = use_interstitial_ad();
    let change_common_settings_callback = use_change_common_settings_callback();

    let onplay = {
        let change_common_settings_callback = change_common_settings_callback.clone();
        let input_ref = input_ref.clone();
        //let set_escaping_callback = ctw.set_escaping_callback.clone();
        let on_play = props.on_play.clone();
        Callback::from(move |_| {
            let new_alias = input_ref
                .cast::<HtmlInputElement>()
                .map(|input| PlayerAlias::new_input_sanitized(&input.value()))
                .filter(|value| !value.is_empty());
            if let Some(new_alias) = new_alias {
                change_common_settings_callback.emit(Box::new(move |settings, storages| {
                    settings.set_alias(Some(new_alias), storages);
                }));
            }
            let alias = new_alias.or(previous_alias).unwrap_or(random_guest_alias);

            let on_play = on_play.clone();
            let start = move |_| {
                // If we wait until set_ui_props sets to in-game mode, activation will have been lost.
                #[cfg(feature = "pointer_lock")]
                crate::request_pointer_lock_with_emulation();
                on_play.emit(alias);
            };

            if let InterstitialAd::Available { request } = &interstitial_ad {
                request.emit(Some(Callback::from(start)));
            } else {
                start(());
            }
        })
    };

    let onclick_play = onplay.reform(|event: MouseEvent| {
        event.prevent_default();
        event.stop_propagation();
    });

    let nav = use_navigation(EngineNexus::PlayWithFriends);
    let set_server_id_callback = ctw.set_server_id_callback.clone();
    let off_public = ctw
        .setting_cache
        .arena_id
        .specific()
        .map(|s| !s.realm_id.is_public_default())
        .unwrap_or(!ctw.setting_cache.arena_id.is_any_instance());
    let server_id = ctw.setting_cache.server_id;
    let core_state = use_core_state();
    let accepted_invitation = core_state.accepted_invitation_id.is_some();
    let invitation_request_callback = use_invitation_request_callback();
    let onclick_play_with_friends = if core_state.accepted_invitation_id.is_some() || off_public {
        Callback::from(move |_event: MouseEvent| {
            if accepted_invitation {
                invitation_request_callback.emit(InvitationRequest::Accept(None));
            }
            if off_public && let Some(server_id) = server_id {
                set_server_id_callback.emit((server_id, ArenaQuery::default()));
            }
        })
    } else {
        nav
    };

    const ENTER: u32 = 13;
    let onkeydown = {
        move |event: KeyboardEvent| {
            if event.key_code() == ENTER {
                event.stop_propagation();
                onplay.emit(());
            }
        }
    };

    let banner_style = css!(
        r#"
        margin: auto;
        min-height: 250px;

        @media (max-height: 800px) {
            min-height: 90px;
        }
        "#
    );

    let promo_style = css!(
        r#"
        width: 15%;
        height: 50%;
        max-width: 20rem;
        max-height: 40rem;

        @media (max-width: 1000px) {
            display: none;
        }
        "#
    );

    let playing_with_friends = accepted_invitation
        || ctw
            .setting_cache
            .arena_id
            .realm_id()
            .is_some_and(|r| !r.is_public_default());

    html! {<>
        <div
            id={"spawn_overlay"}
            class={classes!(form_style, props.animation.then_some(animation_style))}
            style={props.position.to_string()}
            onanimationend={onanimationend.map(|c| c.reform(|_| ())).filter(|_| props.animation)}
        >
            {props.children.clone()}
            if props.beta {
                <b>{translate!(t, "beta", "This game is in BETA; expect to see bugs and balance issues.")}</b>
            }
            <input
                ref={input_ref}
                id="alias_input"
                class={input_style}
                style={props.input_style.clone()}
                disabled={*transitioning}
                {onkeydown}
                type="text"
                minlength="1"
                maxlength="12"
                placeholder={nickname_placeholder(&ctw)}
                autocomplete="off"
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
            />
            <div style="min-width: 12rem; width: min-content; display: flex; flex-direction: column; gap: 1.5rem; margin-top: 0.5rem; margin-bottom: 0.5rem; position: relative; left: 50%; transform: translate(-50%, 0%);">
                <button
                    id="play_button"
                    class={button_style.clone()}
                    style={props.button_style.clone()}
                    disabled={*paused || *transitioning}
                    onclick={onclick_play}
                >{translate!(t, "Play")}</button>
                <button
                    id="play_with_friends_button"
                    class={button_style}
                    style={format!("border: 2px solid #8dd391; font-size: 2rem; padding: 0.4rem 0.5rem; background-color: #549b9f; {}", props.button_style)}
                    disabled={*paused || *transitioning}
                    onclick={onclick_play_with_friends}
                >{format!(
                    "{} {}",
                    if playing_with_friends { '☑' } else { '☐' },
                    translate!(t, "Play with friends")
                )}</button>
            </div>
            <div
                id="banner_container"
                data-instance="splash"
                data-fallback="top"
                class={banner_style}
            ></div>
        </div>
        if features.outbound.promo {
            <Positioner
                id="promo_container"
                position={if props.promo_right_side {
                    Position::CenterRight{margin: "15%"}
                } else {
                    Position::CenterLeft{margin: "0.5rem"}
                }}
                class={promo_style}
            />
        }
    </>}
}

pub fn nickname_placeholder(ctw: &Ctw) -> String {
    let previous_alias = ctw.setting_cache.alias;
    previous_alias
        .filter(|a| !a.is_empty())
        .map(|a| a.as_str().to_owned())
        .unwrap_or(ctw.translator.splash_screen_alias_placeholder().to_owned())
}

/// Should be called on game-specific respawn screens.
#[hook]
pub fn use_splash_screen<E>() -> (
    UseStateHandle<bool>,
    UseStateHandle<bool>,
    Option<Callback<E>>,
) {
    let paused = use_state(|| false);
    let transitioning = use_state(|| true);
    let banner_ad = use_banner_ad();

    let onanimationend = transitioning.then(|| {
        let transitioning = transitioning.clone();
        let banner_ad = banner_ad.clone();
        Callback::from(move |_| {
            // Snippet loaded before splash screen.
            if let BannerAd::Available { request } = &banner_ad {
                request.emit(());
            }
            transitioning.set(false);
        })
    });

    {
        let paused = paused.clone();
        let transitioning = transitioning.clone();
        let banner_ad = banner_ad.clone();

        // See https://yew.rs/docs/concepts/function-components/pre-defined-hooks for why dep is
        // needed.
        let transitioning_dep = *transitioning;

        use_effect_with(
            (transitioning_dep, banner_ad),
            |(currently_transitioning, banner_ad)| {
                let not_transitioning = !*currently_transitioning;
                let banner_ad = banner_ad.clone();
                let listener = GlobalEventListener::new_window(
                    "message",
                    move |event: &MessageEvent| {
                        if let Some(message) = event.data().as_string() {
                            match message.as_str() {
                                "pause" => paused.set(true),
                                "unpause" => paused.set(false),
                                "snippetLoaded" if not_transitioning => {
                                    // Snippet loaded after splash screen.
                                    if let BannerAd::Available { request } = &banner_ad {
                                        request.emit(());
                                    }
                                }
                                _ => {}
                            }
                        }
                    },
                    false,
                );

                // Defend against css animation end event not firing.
                let transition_timeout = not_transitioning
                    .then_some(Timeout::new(1500, move || transitioning.set(false)));

                || {
                    drop(listener);
                    drop(transition_timeout);
                }
            },
        );
    }

    (paused, transitioning, onanimationend)
}
