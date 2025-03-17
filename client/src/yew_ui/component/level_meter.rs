// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{translate, use_translator, Meter};
use yew::{function_component, html, AttrValue, Classes, Html, Properties};

#[derive(PartialEq, Properties)]
pub struct LevelMeterProps {
    #[prop_or(0x0084b1)]
    pub color: u32,
    pub score: u32,
    /// If [`None`], it is inferred.
    pub level: Option<u8>,
    pub score_to_level: fn(u32) -> u8,
    pub level_to_score: fn(u8) -> u32,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(LevelMeter)]
pub fn level_meter(props: &LevelMeterProps) -> Html {
    let current_level = props
        .level
        .unwrap_or_else(|| (props.score_to_level)(props.score));
    let current_level_score = (props.level_to_score)(current_level);
    let max_level = (props.score_to_level)(u32::MAX);
    let next_level = (current_level != max_level).then_some(current_level + 1);
    let next_level_score = next_level.map(props.level_to_score);
    let progress = next_level_score.map(|next_level_score| {
        (props.score.saturating_sub(current_level_score) as f32
            / (next_level_score - current_level_score) as f32)
            .clamp(0.0, 1.0)
    });
    let t = use_translator();
    let upgrade_to_level_progress = |percent: u8, level: u32| -> String {
        translate!(
            t,
            "upgrade_to_level_progress",
            "{percent}% to level {level}"
        )
    };

    html! {
        if let Some((progress, next_level)) = progress.zip(next_level) {
            <Meter
                value={progress}
                class={props.class.clone()}
            >
                {AttrValue::from(upgrade_to_level_progress(
                    // Most of the time, we round. But don't report 100% if we're not there yet.
                    ((progress * 100.0).round() as u8).min(if progress < 1.0 { 99 } else { 100 }),
                    next_level as u32
                ))}
            </Meter>
        }
    }
}
