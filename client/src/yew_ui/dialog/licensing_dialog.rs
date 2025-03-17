// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    markdown, use_ctw, use_features, use_translator, EngineNexus, Link, MarkdownOptions,
    NexusDialog, RoutableExt,
};
use std::rc::Rc;
use yew::{function_component, html, AttrValue, Html};

#[function_component(LicensingDialog)]
pub fn licensing_dialog() -> Html {
    let licenses = use_ctw().licenses;
    let credits = use_features().outbound.credits;
    let t = use_translator();

    let components = Box::new(move |href: &str, content: &str| {
        Some(html! {
            <Link href={href.to_owned()} enabled={credits}>{AttrValue::from(Rc::from(content))}</Link>
        })
    });

    html! {
        <NexusDialog title={EngineNexus::Licensing.label(&t)}>
            {markdown(licenses, &MarkdownOptions{components, ..Default::default()})}
        </NexusDialog>
    }
}
