// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_ctw, use_translator, EngineNexus, NexusDialog, RoutableExt};
use std::borrow::Cow;
use yew::{function_component, html, Html};

#[function_component(StoreDialog)]
pub fn store_dialog() -> Html {
    let ctw = use_ctw();
    let session_id = ctw.setting_cache.session_id;
    let t = use_translator();

    html! {
        <NexusDialog title={EngineNexus::Store.label(&t)}>
            if ctw.setting_cache.store_enabled {
                <iframe
                    style={"border: 0; width: calc(100% - 0.5em); height: calc(100% - 1em);"}
                    src={
                        format!(
                            "https://softbear.com/store/?gameId={}&hideNav&languageId={}{}",
                            ctw.game_constants.game_id,
                            t.language_id,
                            session_id
                                .map(|s| Cow::Owned(format!("&sessionId={}", s.0)))
                                .unwrap_or(Cow::Borrowed(""))
                        )
                    }
                />
            }
        </NexusDialog>
    }
}
