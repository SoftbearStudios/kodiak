// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use kodiak_common::glam::Vec2;
use std::ops::Deref;
use stylist::yew::styled_component;
use web_sys::{HtmlDivElement, Touch, TouchEvent, TouchList};
use yew::{html, use_node_ref, use_state_eq, Callback, Html, NodeRef, Properties, UseStateHandle};

#[derive(PartialEq, Properties)]
pub struct JoystickControllerProps {
    #[prop_or(120)]
    pub size: usize,
    pub onchange: Callback<Vec2>,
}

#[styled_component(JoystickController)]
pub fn joystick(props: &JoystickControllerProps) -> Html {
    let background_ref = use_node_ref();
    let coordinates = use_state_eq(|| Vec2::ZERO);
    let touch_id = use_state_eq(|| Option::<i32>::None);

    // First touch or one matching id
    fn touch_by_id(touches: &TouchList, touch_id: Option<i32>) -> Option<Touch> {
        for i in 0..touches.length() {
            if let Some(touch) = touches.get(i) {
                if touch_id
                    .map(|touch_id| touch.identifier() == touch_id)
                    .unwrap_or(true)
                {
                    return Some(touch);
                }
            } else {
                debug_assert!(false, "failed to get touch");
            }
        }
        None
    }

    fn set_coordinates(
        background_ref: &NodeRef,
        event: &TouchEvent,
        touch_id: i32,
        coordinates: &UseStateHandle<Vec2>,
        onchange: &Callback<Vec2>,
    ) {
        let background = if let Some(background) = background_ref.cast::<HtmlDivElement>() {
            background
        } else {
            debug_assert!(false, "failed to cast background of joystick");
            return;
        };
        let rect = background.get_bounding_client_rect();

        let touches = event.target_touches();

        let (raw_x, raw_y) = if let Some(touch) = touch_by_id(&touches, Some(touch_id)) {
            (touch.client_x() as f32, touch.client_y() as f32)
        } else {
            return;
        };
        let relative_x = raw_x - rect.left() as f32;
        let relative_y = raw_y - rect.top() as f32;

        let coord = (Vec2::new(
            relative_x / rect.width() as f32,
            relative_y / rect.height() as f32,
        ) * 2.0
            - 1.0)
            .clamp_length_max(1.0);

        coordinates.set(coord);
        onchange.emit(coord * Vec2::new(1.0, -1.0));
    }

    let ontouchstart = {
        let touch_id = touch_id.clone();
        Callback::from(move |event: TouchEvent| {
            let touches = event.target_touches();
            if let Some(touch) = touches.get(0) {
                touch_id.set(Some(touch.identifier()));
            }
        })
    };

    let ontouchmove = {
        let background_ref = background_ref.clone();
        let touch_id = touch_id.clone();
        let coordinates = coordinates.clone();
        let onchange = props.onchange.clone();
        touch_id.deref().map(move |touch_id| {
            Callback::from(move |event: TouchEvent| {
                set_coordinates(&background_ref, &event, touch_id, &coordinates, &onchange);
            })
        })
    };

    let ontouchend = {
        let touch_id = touch_id.clone();
        let coordinates = coordinates.clone();
        let onchange = props.onchange.clone();
        Callback::from(move |_: TouchEvent| {
            touch_id.set(None);
            coordinates.set(Vec2::ZERO);
            onchange.emit(Vec2::ZERO);
        })
    };

    let size: usize = props.size;
    let half_size = size / 2;

    let background_style = css!(
        r#"
        border: 4px solid #FFF4;
        border-radius: 50%;
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
    "#
    );

    let stick_style = css!(
        r#"
        border-radius: 50%;
        position: relative;
        background-color: #EEE;
        cursor: move;
        flex-shrink: 0;
        position: absolute;
    "#
    );

    html! {
        <div
            ref={background_ref}
            class={background_style}
            style={format!("width: {size}px; height: {size}px;")}
            {ontouchstart}
            {ontouchmove}
            {ontouchend}
        >
            <div
                class={stick_style}
                style={format!("width: {half_size}px; height: {half_size}px; transform: translate({}px, {}px);", coordinates.x * half_size as f32, coordinates.y * half_size as f32)}
            />
        </div>
    }
}
