// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_translator, EngineNexus, RoutableExt, RouteLink};
use yew::{function_component, html, AttrValue, Html};

#[function_component(PrivacyLink)]
pub fn privacy_link() -> Html {
    let t = use_translator();
    html! {
        <RouteLink<EngineNexus> route={EngineNexus::Privacy}>{AttrValue::from(EngineNexus::Privacy.label(&t))}</RouteLink<EngineNexus>>
    }
}
