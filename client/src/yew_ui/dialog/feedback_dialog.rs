// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_ctw, use_translator, NexusDialog};
use std::borrow::Cow;
use stylist::css;
use yew::{function_component, html, Html};

#[function_component(FeedbackDialog)]
pub fn feedback_dialog() -> Html {
    let ctw = use_ctw();
    let t = use_translator();

    let iframe_style = css!(
        r#"
        border: 0;
        height: 100%;
        left: 0%;
        position: absolute;
        top: 0%;
        width: 100%;
    "#
    );

    html! {
        <NexusDialog close_enabled={true} title={t.feedback_label()}>
            <iframe
                class={iframe_style}
                src={
                    format!(
                        "https://softbear.com/FAQ/#/viewer/{}?hideNav&languageId={}{}",
                        ctw.game_constants.game_id,
                        t.language_id,
                        ctw.setting_cache.session_id
                                    .map(|s| Cow::Owned(format!("&sessionId={}", s.0)))
                                    .unwrap_or(Cow::Borrowed(""))
                    )
                }
            />
        </NexusDialog>
    }
}
