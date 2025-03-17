// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{use_ctw, Curtain, NexusRoute, RouteLink};
use std::rc::Rc;
use stylist::yew::styled_component;
use yew::prelude::*;
use yew_icons::{Icon, IconId};
use yew_router::hooks::use_navigator;
use yew_router::AnyRoute;

#[derive(PartialEq, Properties)]
pub struct NexusDialogProps {
    pub children: Children,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or(false)]
    pub close_enabled: bool,
    #[prop_or(false)]
    pub ok_enabled: bool,
    #[prop_or(None)]
    pub onclick_cancel: Option<Callback<MouseEvent>>,
    #[prop_or(None)]
    pub onclick_ok: Option<Callback<MouseEvent>>,
    #[prop_or(false)]
    pub responsive: bool,
    // TODO: Obsolete inline style.
    #[prop_or("".into())]
    pub style: AttrValue,
    pub title: AttrValue,
}

#[styled_component(NexusDialog)]
pub fn nexus_dialog(props: &NexusDialogProps) -> Html {
    let ctw = use_ctw();
    let t = ctw.translator;

    let dialog_visual_style = css!(
        r#"
        background-color: #174479;
        border-radius: 0.5rem;
        overflow: hidden;
        box-shadow: 5px 5px 5px #00000020;
        color: white;

        button#x {
            background-color: #0000;
            border: none;
            color: rgb(255, 255, 255, 0.7);
            cursor: pointer;
            margin: 0 0 0 auto;
            padding: 0;
            white-space: nowrap;
        }

        button#x:hover {
            color: rgb(255, 255, 255, 1.0);
            transition: 0.7s;
        }

        div#dialog_content {
            overflow-x: unset;
            overflow-y: auto;
        }
    "#
    );

    let absolute_layout_style = css!(
        r#"
        top: 10%;
        left: 10%;
        right: 10%;
        bottom: 10%;
        position: absolute;
        text-align: center;

        @media (max-width: 600px) {
            border-radius: 0;
            top: 0%;
            left: 0%;
            right: 0%;
            bottom: 0%;
        }

        button#x {
            position: absolute;
            right: 0.5rem;
            top: 0.25rem;
        }

        div#action_panel {
            background-color: #1f5da5;
            bottom: 0;
            display: block;
            height: 2rem;
            left: 0;
            padding: 0.5rem;
            position: absolute;
            right: 0;
            text-align: right;
        }

        div#dialog_content {
            bottom: 2rem;
            left: 0;
            padding-left: 0.75rem;
            padding-right: 0.75rem;
            position: absolute;
            right: 0;
            text-align: left;
            top: 2.6rem;
            user-select: text;
        }

        #dialog_footer {
            bottom: 0;
            display: flex;
            height: 2rem;
            justify-content: space-evenly;
            left: 0;
            position: absolute;
            right: 0;
            user-drag: none;
            -webkit-user-drag: none;
        }

        div#dialog_titlebar {
            background-color: #1f5da5;
            height: 2.6rem;
            left: 0;
            position: absolute;
            right: 0;
            top: 0;
        }

        h1, h2, h3, h4, h5, h6 {
            font: red;
            margin-top: 1rem;
            margin-bottom: 0.75rem;
        }

        h2#dialog_title {
            font-size: 1.2rem;
            margin: 0;
            padding: 0.6rem 0 0 0;
        }

        p {
            margin-top: 0.75rem;
            margin-bottom: 0.75rem;
        }
    "#
    );

    let no_footer_style = css!(
        r#"
        #dialog_content {
            height: 90%;  
        }
        "#
    );

    // Assume that `choice_panel` may appear in responsive subclasses.
    let responsive_layout_style = css!(
        r#"
        display: flex;
        flex-direction: column;
        position: absolute;

        @media (max-width: 600px) {
            border-radius: 0 !important;
            height: 100%;
            left: 0%;
            top: 0%;
            width: 100%;

            div#action_panel {
                margin-top: auto;
            }
        }

        @media (min-width: 600px) {
            height: fit-content;
            left: 50%;
            top: 50%;
            transform: translate(-50%, -50%);
            width: fit-content;
        }

        button#x {
            float: right;
        }

        div#action_panel {
            background-color: #1f5da5;
            display: block;
            padding: 0.5rem;
            text-align: right;
        }

        div#choice_panel {
            overflow: unset;
            padding: 0.5rem 1rem 1rem 0.5rem;
        }

        div#dialog_titlebar {
            align-items: center;
            background-color: #1f5da5;
            display: flex;
            padding: 0.5rem;
        }

        h2#dialog_title {
            float: left;
            font-size: 1rem;
            font-weight: bold;
            font-size: 1.2rem;
            line-height: 1.2rem;
            margin: 0;
            padding: 0 2rem 0 0.5rem;
        }

        #dialog_footer {
            display: flex;
            flex-direction: row;
            height: 2rem;
            justify-content: space-evenly;
            user-drag: none;
            -webkit-user-drag: none;
        }
    "#
    );

    let routes = use_ctw().routes;
    let tabs = routes.len() >= 2;

    let dialog_layout_style = if props.responsive {
        classes!(responsive_layout_style)
    } else {
        classes!(
            absolute_layout_style,
            (!(props.close_enabled || props.ok_enabled || tabs)).then_some(no_footer_style)
        )
    };
    let dialog_frame_style = classes!(
        dialog_visual_style,
        dialog_layout_style,
        props.class.clone()
    );

    let absolute_link_style = css!(
        r#"
        align-items: center;
        background-color: #174479;
        display: flex;
        filter: brightness(0.8);
        height: 100%;
        justify-content: center;
        text-decoration: none;
        width: 100%;

        :hover {
            filter: brightness(0.9);
        }
        "#
    );

    let action_button_style = css!(
        r#"
        background-image: linear-gradient(-180deg, #b7d7f1 0%, #77b3e5 100%);
        border: 0;
        border-radius: 0.25rem;
        color: #091c31;
        cursor: pointer;
        font-size: 1rem;
        font-weight: bold;
        padding: 0.25rem 0.5rem 0.25rem 0.5rem;
        margin-right: 1rem;
        touch-action: manipulation;
        user-select: none;

        :disabled {
            color: grey;
        }

        :hover {
            filter: brightness(0.9);
        }
        "#
    );

    let link_selected_style = css!(
        r#"
        background-color: #174479;
        cursor: default;
        filter: none;

        :hover {
            filter: none;
        }
        "#
    );

    let onclick_default = {
        let navigator = use_navigator().unwrap();

        Callback::from(move |_| {
            navigator.push(&AnyRoute::new("/"));
        })
    };
    let onclick_close = props.onclick_cancel.clone().unwrap_or(onclick_default);

    html! {
        <Curtain onclick={onclick_close.clone()}>
            <div id="dialog_frame"
                class={dialog_frame_style}
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                style={props.style.clone()}
                >
                <div id="dialog_titlebar">
                    <h2 id="dialog_title">{props.title.clone()}</h2>
                    <button id="x" onclick={onclick_close.clone()}><Icon icon_id={IconId::HeroiconsMiniSolidXMark}/></button>
                </div>
                <div id="dialog_content">
                    {props.children.clone()}
                </div>
                if tabs {
                    <div id="dialog_footer">
                        {routes.iter().map(|NexusRoute{label, route, selected}| html_nested!{
                            <RouteLink<AnyRoute>
                                route={AnyRoute::new(route)}
                                class={classes!(
                                    absolute_link_style.clone(),
                                    selected.then(|| link_selected_style.clone())
                                )}
                            >
                                    {AttrValue::from(Rc::from(label.as_str()))}
                                </RouteLink<AnyRoute>>
                        }).collect::<Html>()}
                    </div>
                } else if props.close_enabled || props.ok_enabled {
                    <div id="action_panel">
                        if props.ok_enabled {
                            <button disabled={props.onclick_ok.is_none()} onclick={props.onclick_ok.clone()} class={action_button_style.clone()}>{t.ok_label()}</button>
                        }
                        if props.close_enabled {
                            <button onclick={onclick_close} class={action_button_style}>{t.close_label()}</button>
                        } else {
                            <button onclick={onclick_close} class={action_button_style}>{t.cancel_label()}</button>
                        }
                    </div>
                }
            </div>
        </Curtain>
    }
}
