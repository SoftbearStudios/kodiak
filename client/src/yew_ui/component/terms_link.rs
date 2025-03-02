// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_translator, EngineNexus, RoutableExt, RouteLink};
use yew::{function_component, html, AttrValue, Html};

#[function_component(TermsLink)]
pub fn terms_link() -> Html {
    let t: crate::Translator = use_translator();
    html! {
        <RouteLink<EngineNexus> route={EngineNexus::Terms}>{AttrValue::from(EngineNexus::Terms.label(&t))}</RouteLink<EngineNexus>>
    }
}
