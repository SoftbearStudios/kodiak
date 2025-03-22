// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::sprite_sheet::SpriteSheet;
use std::borrow::Cow;
use stylist::yew::styled_component;
use web_sys::MouseEvent;
use yew::virtual_dom::AttrValue;
use yew::{html, Callback, Classes, Html, Properties};

#[derive(Properties, PartialEq)]
pub struct SpriteProps {
    pub sheet: &'static SpriteSheetDetails,
    pub sprite: AttrValue,
    #[prop_or(None)]
    pub title: Option<AttrValue>,
    #[prop_or(None)]
    pub onclick: Option<Callback<MouseEvent>>,
    #[prop_or(None)]
    pub style: Option<AttrValue>,
    #[prop_or(None)]
    pub tint: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

pub struct SpriteSheetDetails {
    pub sprite_sheet: SpriteSheet,
    pub image_src: &'static str,
}

impl PartialEq for SpriteSheetDetails {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

/// static SPRITE_SHEET : LazyLock<kodiak_client::SpriteSheetDetails> = include_sprite_sheet!("./sprites_css.json", "/data/sprites_css.png")
#[macro_export]
macro_rules! include_sprite_sheet {
    ($json: expr, $img: expr) => {
        std::sync::LazyLock::new(|| kodiak_client::SpriteSheetDetails {
            sprite_sheet: serde_json::from_str(include_str!($json)).unwrap(),
            image_src: $img,
        })
    };
}

#[styled_component(Sprite)]
pub fn sprite(props: &SpriteProps) -> Html {
    let sprite = props.sheet.sprite_sheet.sprites.get(props.sprite.as_str());

    if let Some(sprite) = sprite {
        html! {
            <div
                title={props.title.clone()}
                class={props.class.clone()}
                onclick={props.onclick.clone()}
                style={format!(
                    "background-image: url(\"{}\"); background-position: -{}px -{}px; background-clip: content-box; width: {}px; height: {}px;{}{}",
                    props.sheet.image_src,
                    sprite.x,
                    sprite.y,
                    sprite.width,
                    sprite.height,
                    props.tint.as_ref()
                        .map(|t| Cow::Owned(format!("mask-image: url(\"{}\"); background-color: {t}; background-blend-mode: multiply;", props.sheet.image_src)))
                        .unwrap_or(Cow::Borrowed("")),
                    props.style.as_ref().map(|a| a.as_str()).unwrap_or("")
                )}
            />
        }
    } else {
        html! {
            <div
                title={props.title.clone()}
                class={props.class.clone()}
                onclick={props.onclick.clone()}
            />
        }
    }
}
