// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::GlobalEventListener;
use crate::js_hooks::window;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU8;
use wasm_bindgen::JsValue;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct CanvasProps {
    #[prop_or(false)]
    pub blur: bool,
    /// Resolution = window dimension / resolution divisor.
    pub resolution_divisor: NonZeroU8,
    /// Mouse enter, move, down, up, leave.
    pub mouse_callback: Option<Callback<MouseEvent>>,
    /// Touch start, move, end.
    pub touch_callback: Option<Callback<TouchEvent>>,
    /// Focus, blur.
    pub focus_callback: Option<Callback<FocusEvent>>,
    /// Wheel event.
    pub wheel_callback: Option<Callback<WheelEvent>>,
}

pub enum CanvasMsg {
    /// Window size has changed.
    Resize,
}

/// A window-sized canvas element with optional event listeners.
pub struct Canvas {
    _resize_event_listener: GlobalEventListener<Event>,
}

impl Component for Canvas {
    type Message = CanvasMsg;
    type Properties = CanvasProps;

    fn create(ctx: &Context<Self>) -> Self {
        let resize_callback = ctx.link().callback(|_| CanvasMsg::Resize);

        Self {
            _resize_event_listener: GlobalEventListener::new_window(
                "resize",
                move |_event| {
                    resize_callback.emit(());
                },
                false,
            ),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: CanvasMsg) -> bool {
        match msg {
            CanvasMsg::Resize => true,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let w = window();

        let device_pixel_ratio = w.device_pixel_ratio();
        let window_width = dimension(
            w.inner_width(),
            device_pixel_ratio,
            ctx.props().resolution_divisor,
        );
        let window_height = dimension(
            w.inner_height(),
            device_pixel_ratio,
            ctx.props().resolution_divisor,
        );

        struct Blur(bool);

        impl Display for Blur {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                f.write_str(if self.0 {
                    "filter: blur(5px) brightness(0.9);"
                } else {
                    ""
                })
            }
        }

        html! {
            <canvas
                id="canvas"
                style={format!("position: absolute; width: 100%; height: 100%; z-index: -1000; transition: filter 0.15s; user-select: none; {}", Blur(ctx.props().blur))}
                width={window_width}
                height={window_height}
                onmouseenter={ctx.props().mouse_callback.clone()}
                onmousemove={ctx.props().mouse_callback.clone()}
                onmousedown={ctx.props().mouse_callback.clone()}
                onmouseup={ctx.props().mouse_callback.clone()}
                onmouseleave={ctx.props().mouse_callback.clone()}
                ontouchstart={ctx.props().touch_callback.clone()}
                ontouchmove={ctx.props().touch_callback.clone()}
                ontouchend={ctx.props().touch_callback.clone()}
                onwheel={ctx.props().wheel_callback.clone()}
                onblur={ctx.props().focus_callback.clone()}
                onfocus={ctx.props().focus_callback.clone()}
                oncontextmenu={|event: MouseEvent| {
                    event.prevent_default();
                }}
            />
        }
    }
}

fn dimension(
    resolution: Result<JsValue, JsValue>,
    device_pixel_ratio: f64,
    resolution_divisor: NonZeroU8,
) -> String {
    (resolution.unwrap().as_f64().unwrap() * device_pixel_ratio / resolution_divisor.get() as f64)
        .round()
        .to_string()
}
