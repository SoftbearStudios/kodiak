// SPDX-FileCopyrightText: 2024 Softbear, Inc.

use crate::{
    translate, use_ctw, use_game_constants, use_navigation, use_translator, ArenaId, ArenaQuery,
    NexusDialog, RealmId, Sprite, SpriteSheetDetails, Translator,
};
use kodiak_common::SceneId;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use stylist::yew::styled_component;
use yew::{
    classes, html, html_nested, use_state, use_state_eq, Callback, Html, MouseEvent, Properties,
};
use yew_router::hooks::use_navigator;
use yew_router::AnyRoute;

#[derive(Properties, PartialEq)]
pub struct ArenaPickerDialogProps<MID: Ord> {
    pub get_map_id: fn(SceneId) -> MID,
    pub translate_map_id: fn(MID, &Translator) -> String,
    /// Containing preview images.
    pub sheet: &'static SpriteSheetDetails,
}

#[styled_component(ArenaPickerDialog)]
pub fn arena_picker_dialog<MID: Default + Debug + Copy + Ord + 'static>(
    props: &ArenaPickerDialogProps<MID>,
) -> Html {
    let t = use_translator();
    let game_constants = use_game_constants();
    let ctw = use_ctw();
    let default_region_id = ctw.current_region_id();
    let region_id = use_state(|| default_region_id);
    if region_id.is_none() && default_region_id.is_some() {
        region_id.set(default_region_id);
    }
    let map_id = use_state_eq(|| {
        ctw.setting_cache
            .arena_id
            .specific()
            .map(|arena_id| (props.get_map_id)(arena_id.scene_id))
            .unwrap_or_default()
    });

    let instance_id = use_state_eq(|| {
        ctw.setting_cache.server_id.zip(
            ctw.setting_cache
                .arena_id
                .specific()
                .filter(|a| a.realm_id.is_public_default())
                .map(|a| a.scene_id),
        )
    });
    let core_state = ctw.state.as_strong();

    let select_region_id_factory = {
        let region_id = region_id.clone();
        move |new| {
            let region_id = region_id.clone();
            Callback::from(move |_: MouseEvent| {
                region_id.set(Some(new));
            })
        }
    };

    let select_map_id_factory = {
        let map_id = map_id.clone();
        move |new| {
            let map_id = map_id.clone();
            Callback::from(move |_: MouseEvent| {
                map_id.set(new);
            })
        }
    };

    let select_instance_id_factory = {
        let instance_id = instance_id.clone();
        move |server_id, scene_id| {
            let instance_id = instance_id.clone();
            (*instance_id != Some((server_id, scene_id))).then(|| {
                Callback::from(move |_: MouseEvent| {
                    instance_id.set(Some((server_id, scene_id)));
                })
            })
        }
    };

    let on_join = {
        let set_server_id_callback = ctw.set_server_id_callback;
        let navigator = use_navigator().unwrap();
        (*instance_id).map(|(server_id, scene_id)| {
            set_server_id_callback.reform(move |_: MouseEvent| {
                navigator.push(&AnyRoute::new("/"));
                (
                    server_id,
                    ArenaQuery::Specific(ArenaId::new(RealmId::PublicDefault, scene_id), None),
                )
            })
        })
    };
    let on_close = use_navigation(AnyRoute::new("/"));

    let arena_picker_style = classes!(css!(
        r#"
        h3 {
            user-select: none;
        }

        @media (min-width: 600px) {
            div#dialog_content {
                min-width: 20rem;
                min-height: 20rem;
                max-width: 40rem;
                max-height: 30rem;
            }
        }
    "#
    ));

    let row_style = css!(
        r#"
        display: flex;
        flex-direction: row;
        flex-wrap: wrap;
        gap: 1rem;
        user-select: none;
    "#
    );

    let button_style = css!(
        r#"
        background-color: #0075ff;
        border: 2px solid transparent;
        color: white;
        cursor: pointer;
        font-size: 1rem;
        min-height: 2rem;
        min-width: 5rem;
        padding: 0.5rem;
        text-align: center;
        transition: filter 0.25s;
        width: min-content;

        :hover {
            filter: brightness(1.05);
        }
    "#
    );

    let selected_button_style = css!(
        r#"
        border-color:  white;
        cursor: initial;
        filter: brightness(1.1) !important;
    "#
    );

    let regions = {
        let t = t.clone();
        let button_style = button_style.clone();
        let region_id = region_id.clone();
        let selected_button_style = selected_button_style.clone();
        core_state
            .servers
            .values()
            .filter(|s| s.sanctioned).map(|s| s.region_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(move |new| html_nested!{
            <div
                class={classes!(button_style.clone(), (*region_id == Some(new)).then(|| selected_button_style.clone()))}
                onclick={select_region_id_factory(new)}
            >
                {t.region_id(new)}
            </div>
        }).collect::<Html>()
    };

    let maps = {
        let t = t.clone();
        let map_id = map_id.clone();
        let button_style = button_style.clone();
        let selected_button_style = selected_button_style.clone();
        core_state
            .servers
            .values()
            .filter(|s| s.sanctioned && *region_id == Some(s.region_id)).map(|s| (props.get_map_id)(s.scene_id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(move |new| html_nested!{
            <div
                class={classes!(button_style.clone(), (*map_id == new).then(|| selected_button_style.clone()))}
                onclick={select_map_id_factory(new)}
            >
                <Sprite
                    sheet={&props.sheet}
                    sprite={format!("{new:?}")}
                    //style={shuffle_server.is_some().then_some("cursor: pointer;").unwrap_or("")}
                    //onclick={shuffle_server}
                />
                {(props.translate_map_id)(new, &t)}
            </div>
        }).collect::<Html>()
    };

    let instances = {
        let t = t.clone();
        let button_style = button_style.clone();
        let selected_button_style = selected_button_style.clone();
        core_state
            .servers
            .values()
            .filter(|s| s.sanctioned && *region_id == Some(s.region_id) && *map_id == (props.get_map_id)(s.scene_id)).map(|s| ((s.server_id, s.scene_id), s.player_count))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(move |((server_id, scene_id), player_count)| {
                let onclick = select_instance_id_factory(server_id, scene_id);
                html_nested!{
                    <div
                        class={classes!(button_style.clone(), onclick.is_none().then(|| selected_button_style.clone()))}
                        {onclick}
                    >
                        {game_constants.tier_name(server_id.number, scene_id)}
                        <br/>
                        {format!("({})", t.online(player_count as u32))}
                    </div>
            }}).collect::<Html>()
    };

    fn is_empty(html: &Html) -> bool {
        if let Html::VList(list) = html {
            list.is_empty()
        } else {
            false
        }
    }

    let ok_disabled: bool = region_id
        .map(|region_id| {
            !(*instance_id)
                .and_then(|instance| {
                    core_state.servers.get(&instance).map(|s| {
                        s.region_id == region_id
                            && (props.get_map_id)(s.scene_id) == *map_id
                            && s.sanctioned
                    })
                })
                .unwrap_or(false)
        })
        .unwrap_or(true);
    let onclick_ok = on_join.filter(|_| !ok_disabled);
    let onclick_cancel = on_close;

    html! {
        <NexusDialog
            class={arena_picker_style}
            ok_enabled={true}
            {onclick_cancel}
            {onclick_ok}
            responsive={true}
            title={t.find_game_title()}
        >
            <div id="choice_panel">
                if core_state.accepted_invitation_id.is_some() {
                    {translate!(t, "Warning: Selecting a different game will cancel playing with friends.")}
                }
                <h3>
                    {"1. "}
                    {translate!(t, "Select region")}
                </h3>
                <div class={row_style.clone()}>
                    if is_empty(&regions) {
                        <p>{"Loading regions..."}</p>
                    } else {
                        {regions}
                    }
                </div>
                <h3>
                    {"2. "}
                    {translate!(t, "Select map")}
                </h3>
                <div class={row_style.clone()}>
                    if is_empty(&maps) {
                        <p>{translate!(t, "Select a region...")}</p>
                    } else {
                        {maps}
                    }
                </div>
                <h3>
                    {"3. "}
                    {translate!(t, "Select game")}
                </h3>
                <div class={row_style.clone()}>
                    if is_empty(&instances) {
                        <p>{translate!(t, "Select a map...")}</p>
                    } else {
                        {instances}
                    }
                </div>
            </div>
        </NexusDialog>
    }
}
