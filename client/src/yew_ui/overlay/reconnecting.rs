// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{translate, use_translator, Curtain, Position, Positioner, Spinner};
use stylist::yew::styled_component;
use yew::{html, Html};

#[styled_component(Reconnecting)]
pub(crate) fn reconnecting() -> Html {
    let t = use_translator();
    let connection_losing_message = translate!(
        t,
        "connection_losing_message",
        "Connection lost, attempting to reconnect..."
    );
    html! {
        <Curtain>
            <Positioner position={Position::Center}>
                <Spinner/>
                <p>{connection_losing_message}</p>
            </Positioner>
        </Curtain>
    }
}
