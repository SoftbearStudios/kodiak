// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    use_client_request_callback, use_translator, ClientRequest, Position, QuestEvent, TranslateFn,
};
use std::ops::Deref;
use stylist::yew::styled_component;
use web_sys::TransitionEvent;
use yew::{classes, hook, html, use_state, use_state_eq, Callback, Html, Properties};

#[derive(Clone, PartialEq, Properties)]
pub struct InstructionsProps {
    #[prop_or_default]
    pub position: Option<Position>,
    pub primary: Instruction,
    pub secondary: Instruction,
}

pub type Instruction = Option<TranslateFn>;

#[styled_component(Instructions)]
pub fn instructions(props: &InstructionsProps) -> Html {
    let div_style = css!(
        r#"
        pointer-events: none;
        user-select: none;
        color: white;
        "#
    );

    let fade = css!(
        r#"
        opacity: 0.4;
        transition: opacity 0.5s;
        "#
    );

    let active = css!(
        r#"
        opacity: 1.0;
        "#
    );

    let tutorial_started = use_state_eq(|| props.primary.is_some() || props.secondary.is_some());
    let tutorial_started =
        if !*tutorial_started && props.primary.is_some() || props.secondary.is_some() {
            tutorial_started.set(true);
            true
        } else {
            *tutorial_started
        };

    let tutorial_step = use_state_eq(|| 0);
    let new_step = if props.primary.is_none() && props.secondary.is_none() {
        2
    } else if props.primary.is_none() {
        1
    } else {
        0
    };
    let client_request_callback = use_client_request_callback();
    if tutorial_started && new_step > *tutorial_step {
        tutorial_step.set(new_step);

        client_request_callback.emit(ClientRequest::RecordQuestEvent(QuestEvent::Tutorial {
            step: new_step,
        }));
    }

    let t = use_translator();

    #[allow(clippy::type_complexity)]
    #[hook]
    fn use_instruction(instruction: Option<String>) -> (String, Option<Callback<TransitionEvent>>) {
        // Stores the instructions we are transitioning *from*
        // and whether the transition is running.
        let current = use_state::<Option<String>, _>(|| None);
        let fading = use_state(|| false);
        let fade = {
            let current = current.clone();
            let fading = fading.clone();
            Callback::from(move |_| {
                current.set(None);
                fading.set(false);
            })
        };

        if instruction != *current {
            if let Some(current) = current.deref().clone() {
                if !*fading {
                    fading.set(true);
                }
                (current, Some(fade))
            } else {
                current.set(instruction.clone());
                (instruction.unwrap_or_default(), None)
            }
        } else if let Some(new) = instruction {
            (new, None)
        } else {
            (String::new(), Some(fade))
        }
    }

    let (primary, on_primary_transitionend) =
        use_instruction(props.primary.as_ref().map(|f| f(&t)));
    let (secondary, on_secondary_transitionend) =
        use_instruction(props.secondary.as_ref().map(|f| f(&t)));

    html! {
        <div id="instructions" class={div_style} style={props.position.map(|p| p.to_string())}>
            <h2
                style={"font-size: 1.5rem; margin-top: 0.5rem; margin-bottom: 0;"}
                class={classes!(fade.clone(), on_primary_transitionend.is_none().then(|| active.clone()))}
                ontransitionend={on_primary_transitionend}
            >
                {primary}
            </h2>
            <p
                style={"font-size: 1.25rem; margin-top: 0.5rem;"}
                class={classes!(fade, on_secondary_transitionend.is_none().then_some(active))}
                ontransitionend={on_secondary_transitionend}
            >
                {secondary}
            </p>
        </div>
    }
}
