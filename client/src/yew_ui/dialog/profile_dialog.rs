// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_ctw, use_translator, NexusDialog, Position, Positioner, SignInLink};
use std::borrow::Cow;
use yew::{function_component, html, Html};

#[function_component(ProfileDialog)]
pub fn profile_dialog() -> Html {
    let ctw = use_ctw();
    let t = use_translator();

    html! {
        if ctw.features.outbound.accounts.is_some() {
            <NexusDialog title={t.profile_label()}>
                if ctw.setting_cache.user {
                    <iframe
                        style={"border: 0; width: calc(100% - 0.5em); height: calc(100% - 1em);"}
                        src={
                            format!(
                                "https://softbear.com/profile/?gameId={}&hideNav&languageId={}{}{}",
                                ctw.game_constants.game_id,
                                t.language_id,
                                ctw.setting_cache.session_id
                                    .map(|s| Cow::Owned(format!("&sessionId={}", s.0)))
                                    .unwrap_or(Cow::Borrowed("")),
                                    (!ctw.features.outbound.accounts.sign_out()).then_some("&hideSignOut").unwrap_or("")
                            )
                        }
                    />
                } else {
                    <Positioner id={"account"} position={Position::Center}>
                        <SignInLink/>
                    </Positioner>
                }
            </NexusDialog>
        }
    }
}
