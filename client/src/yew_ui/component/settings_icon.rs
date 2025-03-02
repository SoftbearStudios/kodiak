// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_translator, EngineNexus, RouteIcon};
use yew::virtual_dom::AttrValue;
use yew::{function_component, html, Html, Properties};
use yew_icons::IconId;

#[derive(PartialEq, Properties)]
pub struct SettingsIconProps {
    #[prop_or("2rem".into())]
    pub size: AttrValue,
}

#[function_component(SettingsIcon)]
pub fn settings_icon(props: &SettingsIconProps) -> Html {
    let t = use_translator();
    html! {
        <RouteIcon<EngineNexus> icon_id={IconId::BootstrapGear} title={t.settings_title()} route={EngineNexus::Settings} size={props.size.clone()}/>
    }
}
