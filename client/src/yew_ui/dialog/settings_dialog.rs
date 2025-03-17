// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![allow(clippy::type_complexity)]

use crate::{
    post_message, translate, use_core_state, use_ctw, use_gctw, ArenaId, ArenaQuery,
    BrowserStorages, EngineNexus, GameClient, InstancePickerDto, LanguagePicker, LocalSettings,
    NexusDialog, RealmId, RouteLink, SceneId, ServerId, SettingCategory,
};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::ops::RangeInclusive;
use std::str::FromStr;
use stylist::yew::styled_component;
use stylist::StyleSource;
use web_sys::{HtmlInputElement, HtmlSelectElement, InputEvent};
use yew::{classes, html, html_nested, Callback, Html, MouseEvent, TargetCast};

#[styled_component(SettingsDialog)]
pub fn settings_dialog<G: GameClient>() -> Html {
    let ctw = use_ctw();
    let gctw = use_gctw::<G>();
    let t = ctw.translator;

    let select_style = css! {
        r#"
        border-radius: 0.25rem;
        box-sizing: border-box;
        cursor: pointer;
        font-size: 1rem;
        font-weight: bold;
        outline: 0;
        padding: 0.7rem;
        pointer-events: all;
        white-space: nowrap;
        margin-top: 0.25rem;
        margin-bottom: 0.25rem;
        border: 0;
        color: white;
	    background-color: #0075ff;
	    display: block;
        "#
    };

    let slider_style = css!(r#""#);

    fn checkbox<S: 'static>(
        label: String,
        checked: bool,
        callback: fn(&mut S, bool, &mut BrowserStorages),
        change_settings: &Callback<Box<dyn FnOnce(&mut S, &mut BrowserStorages)>>,
    ) -> Html {
        let oninput = change_settings.reform(move |_| {
            Box::new(
                move |settings: &mut S, browser_storages: &mut BrowserStorages| {
                    callback(settings, !checked, browser_storages);
                },
            )
        });

        html! {
            <label
                style="display: block; user-select: none; margin-bottom: 0.6rem;"
            >
                <input type="checkbox" {checked} {oninput}/>
                {label}
            </label>
        }
    }

    fn dropdown<S: 'static>(
        label: String,
        selected: &'static str,
        options: fn(usize) -> Option<(&'static str, &'static str)>,
        callback: fn(&mut S, &str, &mut BrowserStorages),
        change_settings: &Callback<Box<dyn FnOnce(&mut S, &mut BrowserStorages)>>,
        style: &StyleSource,
    ) -> Html {
        let oninput = change_settings.reform(move |event: InputEvent| {
            let string = event.target_unchecked_into::<HtmlSelectElement>().value();
            Box::new(
                move |settings: &mut S, browser_storages: &mut BrowserStorages| {
                    callback(settings, &string, browser_storages);
                },
            )
        });

        let mut n = 0;

        html! {
            <select {oninput} title={label} class={style.clone()}>
                {std::iter::from_fn(move || {
                    let ret = options(n);
                    n += 1;
                    ret
                }).map(|(value, message)| html_nested!(
                    <option {value} selected={value == selected}>{message}</option>
                )).collect::<Html>()}
            </select>
        }
    }

    fn slider<S: 'static>(
        label: String,
        value: f32,
        range: RangeInclusive<f32>,
        callback: fn(&mut S, f32, &mut BrowserStorages),
        change_settings: &Callback<Box<dyn FnOnce(&mut S, &mut BrowserStorages)>>,
        style: &StyleSource,
    ) -> Html {
        let oninput = change_settings.reform(move |event: InputEvent| {
            let string = event.target_unchecked_into::<HtmlInputElement>().value();
            Box::new(
                move |settings: &mut S, browser_storages: &mut BrowserStorages| {
                    if let Ok(value) = f32::from_str(&string) {
                        callback(settings, value, browser_storages);
                    }
                },
            )
        });
        html! {
            <label
                style="display: block; user-select: none; margin-bottom: 0.6rem;"
            >
                <input
                    type="range"
                    min={range.start().to_string()}
                    max={range.end().to_string()}
                    step={0.01}
                    value={value.to_string()}
                    {oninput}
                    class={style.clone()}
                />
                {label}
            </label>
        }
    }

    let core_state = use_core_state();
    let selected_server_id = ctw
        .setting_cache
        .server_id
        .map(|s| (s, ctw.setting_cache.arena_id.specific().unwrap_or_default()));
    let on_select_server_id = {
        let set_server_id_callback = ctw.set_server_id_callback;
        Callback::from(move |event: InputEvent| {
            let value = event.target_unchecked_into::<HtmlSelectElement>().value();
            let Some((server_id, scene_id)) = value.rsplit_once('/') else {
                return;
            };
            let Ok(server_id) = ServerId::from_str(server_id) else {
                return;
            };
            let Ok(scene_id) = SceneId::from_str(scene_id) else {
                return;
            };
            set_server_id_callback.emit((
                server_id,
                ArenaQuery::Specific(
                    ArenaId {
                        realm_id: RealmId::PublicDefault,
                        scene_id,
                    },
                    None,
                ),
            ));
        })
    };

    let categories = std::cell::RefCell::new(BTreeMap::<
        SettingCategory,
        BTreeMap<Cow<'static, str>, Html>,
    >::new());

    categories
        .borrow_mut()
        .entry(SettingCategory::General)
        .or_default()
        .insert(
            Cow::Borrowed("Language"),
            html! {
                <LanguagePicker always_open={true} override_class={classes!(select_style.clone())}/>
            },
        );

    if ctw.features.ad_privacy {
        categories
            .borrow_mut()
            .entry(SettingCategory::Privacy)
            .or_default()
            .insert(
                Cow::Borrowed("Advertisements"),
                html! {
                    <button
                        onclick={|e: MouseEvent| {
                            e.prevent_default();
                            post_message("requestAdPrivacy");
                        }}>{"Ad Privacy"}</button>
                },
            );
    }
    let _volume = translate!(t, "Volume");
    #[cfg(feature = "audio")]
    categories.borrow_mut().entry(SettingCategory::Audio).or_default().insert(Cow::Borrowed("Volume"), html!{
        <label
            style="display: block; user-select: none; margin-bottom: 0.6rem; line-height: 1.75rem;"
        >
            <crate::VolumePicker/>
            {_volume}
        </label>
    });
    let mut server_regions = BTreeMap::<Cow<'static, str>, Vec<&InstancePickerDto>>::new();
    for server in core_state.servers.values() {
        let region_str = if server.server_id.kind.is_local() {
            Cow::Borrowed("Local")
        } else {
            Cow::Owned(t.region_id(server.region_id))
        };
        let region = server_regions.entry(region_str).or_default();
        region.push(server);
    }
    // _ to go last
    categories.borrow_mut().entry(SettingCategory::General).or_default().insert(Cow::Borrowed("_Server"), html!{
        <select
            oninput={on_select_server_id}
            class={select_style.clone()}
        >
            {server_regions.into_iter().map(|(region_id, instances)| html!{<>
                <option disabled={true}>{format!("-- {region_id} --")}</option>
                {instances.into_iter().map(|&InstancePickerDto{server_id, scene_id, player_count, sanctioned, ..}| {
                    let mut name = G::GAME_CONSTANTS.tier_name(server_id.number, scene_id);
                    if let Some(description) = G::describe_scene_id(scene_id) {
                        write!(&mut name, " \"{}\"", description(&t)).unwrap();
                    }
                    let label = format!("{}-{name} ({})", server_id.number, t.online(player_count as u32));
                    html_nested!{
                        <option
                            value={format!(
                                "{server_id}/{}",
                                scene_id.to_string(),
                            )}
                            disabled={!sanctioned}
                            selected={selected_server_id == Some((server_id, ArenaId{realm_id: RealmId::PublicDefault, scene_id}))}
                        >
                            {label}
                        </option>
                    }
                }).collect::<Html>()}
            </>}).collect::<Html>()}
            <option disabled={true}>{"-- Other --"}</option>
            if let Some((server_id, ArenaId{realm_id: RealmId::Temporary(temporary_realm_index), ..})) = selected_server_id {
                <option value="temporary" selected={true}>{format!("{}-{}/party/{temporary_realm_index}", server_id.number, G::GAME_CONSTANTS.server_name(server_id.number))}</option>
            } else {
                if selected_server_id.map(|(s, a)| !core_state.servers.contains_key(&(s, a.scene_id))).unwrap_or(true) {
                    <option value="unknown" selected={true}>{"Unknown server"}</option>
                }
            }
        </select>
    });
    gctw.settings_cache.display(
        &t,
        |a, b, c, d| {
            categories.borrow_mut().entry(a).or_default().insert(
                Cow::Owned(b.clone()),
                checkbox(b, c, d, &gctw.change_settings_callback),
            );
        },
        |a, b, c, d, e| {
            categories.borrow_mut().entry(a).or_default().insert(
                Cow::Owned(b.clone()),
                dropdown(b, c, d, e, &gctw.change_settings_callback, &select_style),
            );
        },
        |a, b, c, d, e| {
            categories.borrow_mut().entry(a).or_default().insert(
                Cow::Owned(b.clone()),
                slider(b, c, d, e, &gctw.change_settings_callback, &slider_style),
            );
        },
    );
    ctw.setting_cache.display(
        &t,
        |a, b, c, d| {
            if a == SettingCategory::Privacy && !ctw.features.cookie_consent {
                return;
            }
            if !ctw.features.chat && b == "Chat" {
                return;
            }
            categories.borrow_mut().entry(a).or_default().insert(
                Cow::Owned(b.clone()),
                checkbox(b, c, d, &ctw.change_common_settings_callback),
            );
        },
        |a, b, c, d, e| {
            if a == SettingCategory::Privacy && !ctw.features.cookie_consent {
                return;
            }
            categories.borrow_mut().entry(a).or_default().insert(
                Cow::Owned(b.clone()),
                dropdown(
                    b,
                    c,
                    d,
                    e,
                    &ctw.change_common_settings_callback,
                    &select_style,
                ),
            );
        },
        |a, b, c, d, e| {
            if a == SettingCategory::Privacy && !ctw.features.cookie_consent {
                return;
            }
            categories.borrow_mut().entry(a).or_default().insert(
                Cow::Owned(b.clone()),
                slider(
                    b,
                    c,
                    d,
                    e,
                    &ctw.change_common_settings_callback,
                    &slider_style,
                ),
            );
        },
    );

    html! {
        <NexusDialog title={t.settings_title()}>
            <p>
                {"By changing settings, you consent to cookies being stored in accordance with our "}
                <RouteLink<EngineNexus> route={EngineNexus::Privacy}>{"privacy policy"}</RouteLink<EngineNexus>>
                {"."}
            </p>
            {categories
                .into_inner()
                .into_iter()
                .map(|(category, settings)| {
                html!{<>
                    <h3>{t.setting_category(category)}</h3>
                    {settings.into_values().collect::<Html>()}
                </>}
            }).collect::<Html>()}
        </NexusDialog>
    }
}
