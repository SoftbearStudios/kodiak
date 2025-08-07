// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    event_target, high_contrast_class, profile_factory, translate, use_chat_request_callback,
    use_core_state, use_ctw, use_set_context_menu_callback, use_translator, ArenaId,
    BrowserStorages, ChatMessage, ChatRequest, CommonSettings, ContextMenu, ContextMenuButton,
    GlobalEventListener, Position, Positioner, RealmId, ServerNumber, Translator,
};
use js_sys::JsString;
use std::str::pattern::Pattern;
use stylist::yew::styled_component;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, InputEvent, KeyboardEvent, MouseEvent};
use yew::{
    classes, html, html_nested, use_effect_with, use_node_ref, use_state_eq, AttrValue, Callback,
    Html, Properties,
};

#[derive(PartialEq, Properties)]
pub struct ChatProps {
    /// Override the default label.
    #[prop_or(Translator::chat_label)]
    pub label: fn(&Translator) -> String,
    pub position: Position,
    #[prop_or(None)]
    pub style: Option<AttrValue>,
    #[prop_or_default]
    pub hints: &'static [(&'static str, &'static [&'static str])],
}

#[styled_component(ChatOverlay)]
pub fn chat_overlay(props: &ChatProps) -> Html {
    let container_style = css!(
        r#"
        max-width: 25%;

        :hover > .message, :focus-within > .message {
            opacity: 1 !important;
            user-select: text !important;
            pointer-events: initial !important;
            display: block !important;
            line-height: 1.0 !important;
            margin-bottom: 0.25rem !important;
        }
        "#
    );

    let message_css_class = css!(
        r#"
        animation-name: fadeOut;
        animation-duration: 5s;
        animation-delay: 20s;
        animation-fill-mode: forwards;
        animation-timing-function: linear;
        color: white;
        margin-bottom: 0.25rem;
        margin-top: 0;
        overflow-wrap: anywhere;
        text-overflow: ellipsis;
        word-break: normal;
        user-select: text;
        text-align: left;
        pointer-events: auto;
        line-height: 1.0;

        @keyframes fadeOut {
            0% {
                opacity: 1;
            }
            99% {
                opacity: 0;
                line-height: 1.0;
                margin-bottom: 0.25rem;
                pointer-events: initial;
                user-select: text;
            }
            100% {
                opacity: 0;
                user-select: none;
                pointer-events: none;
                line-height: 0;
                margin-bottom: 0;
            }
        }       
        "#
    );

    let whisper_style = css!(
        r#"
        filter: brightness(0.7);
        "#
    );

    let name_css_class = css!(
        r#"
        font-weight: bold;
        white-space: nowrap;
        user-select: none;
    "#
    );

    let authentic_css_class = css!(
        r#"
        font-style: italic;
    "#
    );

    let official_name_css_class = css!(
        r#"
        cursor: initial;
        font-weight: bold;
        white-space: nowrap;
        color: #fffd2a;
        text-shadow: 0px 0px 3px #381616;
        user-select: none;
        "#
    );

    let no_select_style = css!(
        r#"
        user-select: none;
        "#
    );

    let clickable_style = css!(
        r#"
        cursor: pointer;
        "#
    );

    let mention_style = css!(
        r#"
        color: #cae3ec;
        font-weight: bold;
        background: #63ccee3d;
        border-radius: 0.25rem;
        padding: 0.1rem 0.15rem;
        "#
    );

    let input_css_class = css!(
        r#"
        border-radius: 0.25rem;
        box-sizing: border-box;
        cursor: pointer;
        font-size: 1rem;
        font-weight: bold;
        outline: 0;
        padding: 0.5rem;
        white-space: nowrap;
        margin-top: 0.25rem;
        background-color: #00000025;
        border: 0;
        color: white;
        width: 100%;
        "#
    );

    let ctw = use_ctw();
    let high_contrast_class = high_contrast_class!(ctw, css);

    let on_save_chat_message = ctw.change_common_settings_callback.reform(|chat_message| {
        Box::new(
            move |common_settings: &mut CommonSettings, browser_storages: &mut BrowserStorages| {
                common_settings.set_chat_message(chat_message, browser_storages);
            },
        )
    });

    let t = use_translator();
    let input_ref = use_node_ref();
    let help_hint = use_state_eq::<Option<&'static str>, _>(|| None);
    let is_command = use_state_eq(|| false);

    let oninput = {
        let help_hint = help_hint.clone();
        let is_command = is_command.clone();
        let hints = props.hints;
        let on_save_chat_message = on_save_chat_message.clone();

        move |event: InputEvent| {
            let input: HtmlInputElement = event_target(&event);
            let string = input.value();
            help_hint.set(help_hint_of(hints, &string));
            is_command.set(string.starts_with('/'));
            on_save_chat_message.emit(string);
        }
    };

    const ENTER: u32 = 13;

    let chat_request_callback = use_chat_request_callback();

    let onkeydown = {
        let help_hint = help_hint.clone();
        let is_command = is_command.clone();
        let chat_request_callback = chat_request_callback.clone();

        move |event: KeyboardEvent| {
            if event.key_code() != ENTER {
                return;
            }
            event.stop_propagation();
            let input: HtmlInputElement = event_target(&event);
            let mut message = input.value();
            input.set_value("");
            let _ = input.blur();
            let mut whisper = event.shift_key();
            if let Some(inner) = message.strip_prefix("/t ") {
                message = inner.to_owned();
                whisper = true;
            }
            if message.is_empty() {
                return;
            }
            chat_request_callback.emit(ChatRequest::Send { message, whisper });
            on_save_chat_message.emit(String::new());
            help_hint.set(None);
            is_command.set(false);
        }
    };

    fn focus(input: &HtmlInputElement) {
        // Want the UTF-16 length;
        let string: JsString = input.value().into();
        let length = string.length();
        let _ = input.focus();
        let _ = input.set_selection_range(length, length);
    }

    // Pressing Enter key focuses the input.
    {
        let input_ref = input_ref.clone();
        let default_text = ctw.setting_cache.chat_message.clone();

        use_effect_with((input_ref, default_text), |(input_ref, default_text)| {
            let input_ref = input_ref.clone();

            if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                input.set_value(default_text);
            }

            let onkeydown = GlobalEventListener::new_window(
                "keydown",
                move |e: &KeyboardEvent| {
                    const SLASH: u32 = 191;
                    if matches!(e.key_code(), ENTER | SLASH)
                        && !e
                            .target()
                            .map(|t| t.is_instance_of::<HtmlInputElement>())
                            .unwrap_or(false)
                    {
                        if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                            focus(&input);
                        }
                    }
                },
                false,
            );

            move || drop(onkeydown)
        });
    }

    let core_state = use_core_state();
    let set_context_menu_callback = use_set_context_menu_callback();
    let profile_factory = profile_factory(&ctw);
    let (mention_string, moderator) = core_state
        .player()
        .map(|p| (format!("@{}", p.alias), p.moderator))
        .unwrap_or((String::from("PLACEHOLDER"), false));

    let items = core_state.messages.iter().map(|(message_number, dto)| {
        let message_number = *message_number;
        let onclick_reply = {
            let input_ref_clone = input_ref.clone();
            let alias = dto.alias;
            move || {
                if let Some(input) = input_ref_clone.cast::<HtmlInputElement>() {
                    let mut message = input.value();
                    let mention = format!("@{} ", alias.as_str());
                    if !message.ends_with(&mention) {
                        message.push_str(&mention);
                        input.set_value(&message);
                    }
                    focus(&input);
                }
            }
        };

        let (authentic, oncontextmenu) = if dto.authority {
            (false, None)
        } else {
            let visitor_id = dto.visitor_id;
            let t = t.clone();
            let chat_request_callback = chat_request_callback.clone();
            let set_context_menu_callback = set_context_menu_callback.clone();
            let profile_factory = profile_factory.clone();
            let chat_restrict_5m_label = AttrValue::from(translate!(t, "Restrict (5m+)"));
            let chat_report_label = AttrValue::from(translate!(t, "Report"));
            let chat_mute_label = AttrValue::from(translate!(t, "Mute"));

            let oncontextmenu = Some(move |e: MouseEvent| {
                e.prevent_default();
                e.stop_propagation();
                let t = t.clone();
                let chat_request_callback = chat_request_callback.clone();
                let profile_factory = profile_factory.clone();
                let onclick_mute = {
                    let chat_request_callback = chat_request_callback.clone();
                    Callback::from(move |_: MouseEvent| {
                        chat_request_callback.emit(ChatRequest::Mute(message_number));
                    })
                };
                let onclick_report = {
                    let chat_request_callback = chat_request_callback.clone();
                    Callback::from(move |_: MouseEvent| {
                        chat_request_callback.emit(ChatRequest::Report(message_number));
                    })
                };

                let html = html!{
                    <ContextMenu position={&e}>
                        if visitor_id.is_some() {
                            <ContextMenuButton onclick={profile_factory(visitor_id)}>{AttrValue::from(t.profile_label())}</ContextMenuButton>
                        }
                        <ContextMenuButton onclick={onclick_mute}>{chat_mute_label.clone()}</ContextMenuButton>
                        <ContextMenuButton onclick={onclick_report}>{
                            if moderator {
                                chat_restrict_5m_label.clone()
                            } else {
                                chat_report_label.clone()
                            }
                        }</ContextMenuButton>
                    </ContextMenu>
                };
                set_context_menu_callback.emit(Some(html));
            });

            (dto.authentic, oncontextmenu)
        };

        let format_arena = |server_number: ServerNumber, arena_id: ArenaId| -> String {
            match arena_id.realm_id {
                RealmId::Temporary(index) => format!(
                    "{}/party/{index}",
                    ctw.game_constants.server_name(server_number)
                ),
                _ => ctw.game_constants.tier_name(server_number, arena_id.scene_id),
            }
        };

        html_nested!{
            <p
                key={message_number}
                class={
                    classes!(
                        "message",
                        message_css_class.clone(),
                        dto.whisper.then(|| whisper_style.clone()),
                    )
                }
                {oncontextmenu}
            >
                if let Some(team_name) = dto.team_name {
                    <span class={name_css_class.clone()}>
                        {"["}
                        {team_name}
                        {"] "}
                    </span>
                }
                if !matches!(dto.message, ChatMessage::Join{..}) {
                    <span
                        onclick={move |_| onclick_reply()}
                        class={classes!(
                            name_css_class.clone(),
                            authentic.then(|| authentic_css_class.clone()),
                            (!dto.authority).then(|| clickable_style.clone()),
                            dto.authority.then(|| official_name_css_class.clone()),
                        )}
                    >
                        {dto.alias.as_str()}
                    </span>
                    <span class={no_select_style.clone()}>{" "}</span>
                }
                {match &dto.message {
                    ChatMessage::Raw{message, detected_language_id, english_translation} => {
                        let message = if let Some(english_translation) = english_translation && *detected_language_id != ctw.setting_cache.language {
                            english_translation
                        } else {
                            message
                        };
                        segments(message, &mention_string).map(|Segment{contents, mention}| html_nested!{
                            <span
                                class={classes!(mention.then(|| mention_style.clone()))}
                                // e.g. long netquel codes with no spaces
                                style={(message.split_ascii_whitespace().map(|w| w.len()).max().unwrap_or(0) > 20).then_some("word-break:break-all;")}
                            >{contents.to_owned()}</span>
                        }).collect::<Html>()
                    },
                    &ChatMessage::Welcome{server_number, arena_id} => {
                        let arena = format_arena(server_number, arena_id);
                        html!{<span>
                            {translate!(t, "Welcome to {arena}! Remember to guard your privacy in chat and abide by our terms.")}
                        </span>}
                    }
                    &ChatMessage::Join{alias, authentic, rank, visitor_id, server_number, arena_id} => {
                        let arena = format_arena(server_number, arena_id);
                        html!{<span>
                            if let Some(rank) = rank {
                                {(ctw.translate_rank_number)(&t, rank)}
                            } else {
                                {translate!(t, "Unranked")}
                            }
                            {" "}
                            <span
                                class={classes!(
                                    name_css_class.clone(),
                                    authentic.then(|| authentic_css_class.clone()),
                                    clickable_style.clone(),
                                )}
                                onclick={profile_factory(visitor_id)}
                            >
                                {alias.as_str()}
                            </span>
                            {" "}
                            {translate!(t, "joined {arena}")}
                        </span>}
                    }
                    ChatMessage::SignInOrDisableVpn => html!{<span>
                        {translate!(t, "Either sign in or disable your VPN to chat")}
                    </span>}
                }}
            </p>
        }
    }).collect::<Html>();

    let title = if core_state.team_id().is_some() {
        t.chat_send_team_message_hint()
    } else {
        t.chat_send_message_hint()
    };

    html! {
        if ctw.setting_cache.chat && ctw.features.chat {
            <Positioner
                id="chat"
                position={props.position}
                style={props.style.clone()}
                class={classes!(container_style, high_contrast_class)}
            >
                {items}
                if let Some(help_hint) = *help_hint {
                    <p><b>{"Automated help: "}{help_hint}</b></p>
                }
                <input
                    type="text"
                    name="message"
                    {title}
                    {oninput}
                    {onkeydown}
                    autocomplete="off"
                    minLength="1"
                    maxLength={
                        if *is_command {
                            "8192"
                        } else {
                            "128"
                        }
                    }
                    placeholder={t.chat_send_message_placeholder()}
                    class={input_css_class.clone()}
                    ref={input_ref}
                />
            </Positioner>
        }
    }
}

fn help_hint_of(
    hints: &[(&'static str, &'static [&'static str])],
    text: &str,
) -> Option<&'static str> {
    let text = text.to_ascii_lowercase();
    if text.contains("/invite") {
        Some("Invitation links cannot currently be accepted by players that are already in game. They must send a join request instead.")
    } else {
        for (value, keys) in hints.iter() {
            if keys.iter().all(|&k| {
                debug_assert_eq!(k, k.to_ascii_lowercase());
                text.contains(k)
            }) {
                return Some(value);
            }
        }
        None
    }
}

#[derive(Debug)]
struct Segment<'a> {
    pub contents: &'a str,
    pub mention: bool,
}

fn segments<'a, P: Pattern<'a> + Clone>(message: &'a str, mention: P) -> Segments<'a, P> {
    Segments { message, mention }
}

struct Segments<'a, P: Pattern<'a> + Clone> {
    message: &'a str,
    mention: P,
}

impl<'a, P: Pattern<'a> + Clone> Iterator for Segments<'a, P> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.message.is_empty() {
            // We are done.
            None
        } else {
            let (idx, mtch) = self
                .message
                .match_indices(self.mention.clone())
                .next()
                .unwrap_or((self.message.len(), self.message));
            if idx == 0 {
                // Mention is at the beginning, return it.
                let (before, after) = self.message.split_at(mtch.len());
                if before.is_empty() {
                    // Guard against empty pattern.
                    self.message = "";
                    return Some(Segment {
                        contents: after,
                        mention: false,
                    });
                }
                self.message = after;
                Some(Segment {
                    contents: before,
                    mention: true,
                })
            } else {
                // Mention is later on, return the non-mention before it.
                let (before, after) = self.message.split_at(idx);
                self.message = after;
                Some(Segment {
                    contents: before,
                    mention: false,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{segments, Segment};
    use kodiak_common::rand::prelude::SliceRandom;
    use kodiak_common::rand::{thread_rng, Rng};

    #[test]
    fn fuzz_segments() {
        fn random_string() -> String {
            std::iter::from_fn(|| ['a', '大', 'π'].choose(&mut thread_rng()))
                .take(thread_rng().gen_range(0..=12))
                .collect()
        }

        for _ in 0..200000 {
            let message = random_string();
            let mention = random_string();

            // Make sure it terminates, conserves characters, and doesn't return empty contents or
            // repeat non-mentions.
            let mut total = 0;
            let mut mentioned = true;
            for Segment { contents, mention } in segments(&message, &mention) {
                debug_assert!(!contents.is_empty());
                total += contents.len();
                if mention {
                    mentioned = true;
                } else {
                    debug_assert!(mentioned);
                    mentioned = false;
                }
            }
            debug_assert_eq!(message.len(), total);
        }
    }
}
