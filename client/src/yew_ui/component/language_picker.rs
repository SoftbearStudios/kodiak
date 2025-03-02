// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    event_target, translate, use_change_common_settings_callback, use_ctw, use_translator,
    LanguageId,
};
use gloo::timers::callback::Timeout;
use std::str::FromStr;
use stylist::yew::styled_component;
use web_sys::{Event, HtmlSelectElement};
use yew::{classes, html, html_nested, use_state, Callback, Classes, Html, Properties};
use yew_icons::{Icon, IconId};

#[derive(PartialEq, Properties)]
pub struct LanguagePickerProps {
    #[prop_or(false)]
    pub always_open: bool,
    #[prop_or(None)]
    pub override_class: Option<Classes>,
}

#[styled_component(LanguagePicker)]
pub fn language_picker(props: &LanguagePickerProps) -> Html {
    let ctw = use_ctw();
    // Open if [`Some`], closed otherwise. The [`Some`] variant stores a timer to close it automatically.
    let menu_open = use_state::<Option<Timeout>, _>(|| None);

    let div_css_class = css!(
        r#"
        height: 2rem;
        position: relative;
        width: 2rem;
    "#
    );

    let select_css_class = css!(
        r#"
        background-color: #CCC;
        color: black;
        position: absolute;
        right: 0;
        top: 0;
        width: min-content;
        border-radius: 0.25rem;
        box-sizing: border-box;
        cursor: pointer;
        font-size: 0.8rem;
        font-weight: bold;
        outline: 0;
        padding: 0.7rem;
        pointer-events: all;
        white-space: nowrap;
        margin-top: 0.25rem;
        border: 0;
    "#
    );

    let handle_open = {
        let menu_open = menu_open.clone();

        Callback::from(move |_| {
            if menu_open.is_none() {
                let menu_open_clone = menu_open.clone();

                menu_open.set(Some(Timeout::new(10000, move || {
                    menu_open_clone.set(None);
                })));
            };
        })
    };

    let handle_change = {
        let change_common_settings_callback = use_change_common_settings_callback();
        let menu_open = menu_open.clone();

        move |event: Event| {
            let select: HtmlSelectElement = event_target(&event);
            let value = LanguageId::from_str(&select.value());
            if let Ok(value) = value {
                change_common_settings_callback.emit(Box::new(
                    move |common_settings, browser_storage| {
                        common_settings.set_language(value, browser_storage);
                    },
                ));
            }
            menu_open.set(None);
        }
    };

    let t = use_translator();

    let select = || {
        html! {
            <select onchange={handle_change} class={props.override_class.clone().unwrap_or_else(|| classes!(select_css_class))}>
                {t.languages.iter().map(|dto| {
                    html_nested!{
                        <option
                            value={dto.language_id.to_string()}
                            selected={dto.language_id == ctw.setting_cache.language}
                        >{dto.language_name.clone()}</option>
                    }
                }).collect::<Html>()}
            </select>
        }
    };

    if props.always_open {
        select()
    } else {
        html! {
            if menu_open.is_some() {
                <div class={div_css_class}>
                    {select()}
                </div>
            } else {
                <Icon
                    icon_id={IconId::BootstrapGlobe2}
                    width={String::from("2rem")}
                    height={String::from("1.8rem")}
                    title={translate!(t, "Language")}
                    onclick={handle_open}
                    style={"cursor: pointer;"}
                />
            }
        }
    }
}
