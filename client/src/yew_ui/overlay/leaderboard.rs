// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    high_contrast_class, is_mobile, profile_factory, translate, use_core_state, use_ctw,
    use_translator, BrowserStorages, CommonSettings, LeaderboardCaveat, PeriodId, Position,
    Positioner,
};
use stylist::yew::styled_component;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct LeaderboardProps {
    pub position: Position,
    #[prop_or(None)]
    pub style: Option<AttrValue>,
    #[prop_or(false)]
    pub liveboard: bool,
    /// Set to false to hide all-time i.e. if the scoring isn't final.
    #[prop_or(true)]
    pub all_time: bool,
    #[prop_or(0x8ce8fd)]
    pub your_score_color: u32,
    /// Override the default leaderboard label.
    #[prop_or(LeaderboardProps::fmt_precise)]
    pub fmt_score: fn(u32) -> String,
}

impl LeaderboardProps {
    pub fn fmt_precise(score: u32) -> String {
        score.to_string()
    }

    pub fn fmt_abbreviated(score: u32) -> String {
        let power = score.max(1).ilog(1000);
        if power == 0 {
            score.to_string()
        } else {
            let units = ["", "k", "m", "b"];
            let power_of_1000 = 1000u32.pow(power);
            let unit = units[power as usize];
            // TODO: Round down not up.
            let fraction = score as f32 / power_of_1000 as f32;
            format!("{:.1}{}", fraction, unit)
        }
    }
}

// TODO: delete props.show_my_score
#[styled_component(LeaderboardOverlay)]
pub fn leaderboard_overlay(props: &LeaderboardProps) -> Html {
    let p_css_class = css!(
        r#"
        color: white;
        font-style: italic;
        margin-bottom: 0rem;
        margin-top: 0.5rem;
        text-align: center;
    "#
    );

    let table_css_class = css!(
        r#"
        color: white;
        width: 13rem;
        max-width: 100%;
        line-height: 120%;

        td.name {
            font-weight: bold;
            text-align: left;
            white-space: nowrap;
            text-overflow: ellipsis;
            overflow: hidden;
            max-width: 10vw;
        }

        td.ranking {
            text-align: right;
        }

        td.score {
            text-align: right;
        }
    "#
    );

    let ctw = use_ctw();
    let high_contrast_class = high_contrast_class!(ctw, css);
    let change_common_settings_callback = ctw.change_common_settings_callback.clone();
    let change_period_factory = move |period_id: PeriodId| {
        change_common_settings_callback.reform(move |event: MouseEvent| {
            event.prevent_default();
            event.stop_propagation();
            Box::new(
                move |common_settings: &mut CommonSettings,
                      browser_storages: &mut BrowserStorages| {
                    common_settings.set_leaderboard_period_id(period_id, browser_storages);
                },
            )
        })
    };

    let t = use_translator();
    let core_state = use_core_state();
    let profile_factory = profile_factory(&ctw);

    let count = if is_mobile() { 5 } else { 10 };

    let (items, footer) = if props.liveboard {
        let extra = core_state
            .your_score
            .as_ref()
            .map(|your_score| (your_score.ranking as usize, your_score.inner.clone()));

        let mut items = core_state
            .liveboard
            .iter()
            .enumerate()
            .filter(|(i, _)| extra.as_ref().map(|(j, _)| i != j).unwrap_or(true))
            .map(|(i, dto)| (i, dto.clone()))
            .take(count - extra.is_some() as usize)
            .collect::<Vec<_>>();

        if let Some(extra) = extra.as_ref() {
            let index = items
                .iter()
                .position(|(rank, _)| *rank > extra.0)
                .unwrap_or(usize::MAX)
                .min(items.len());
            items.insert(index, extra.clone())
        }
        let items = items
            .into_iter()
            .map(|(ranking, dto)| {
                let profile = dto
                    .visitor_id
                    .is_some()
                    .then(|| profile_factory(dto.visitor_id))
                    .flatten();
                html_nested! {
                    <tr
                        style={extra
                            .as_ref()
                            .and_then(|(r, _)|
                                (*r == ranking)
                                    .then(|| format!("color: #{:06x};", props.your_score_color))
                            )
                        }
                    >
                        <td class="ranking">{ranking + 1}{"."}</td>
                        <td
                            class="name"
                            style={format!(
                                "{}{}",
                                dto.authentic.then_some("font-style: italic;").unwrap_or(""),
                                profile.is_some().then_some("pointer-events: auto;").unwrap_or(""),
                            )}
                            onclick={profile.clone()}
                            oncontextmenu={profile}
                        >{dto.alias.fmt_with_team_name(dto.team_name)}</td>
                        <td class="score">{(props.fmt_score)(dto.score)}</td>
                    </tr>
                }
            })
            .collect::<Html>();

        let players = core_state.players_on_shard;
        let arena = {
            ctw.setting_cache
                .server_id
                .map(|s| {
                    if core_state.shard_per_scene {
                        ctw.game_constants.tier_name(
                            s.number,
                            ctw.setting_cache
                                .arena_id
                                .specific()
                                .unwrap_or_default()
                                .scene_id,
                        )
                    } else {
                        ctw.game_constants.server_name(s.number).to_owned()
                    }
                })
                .unwrap_or_else(|| "???".to_owned())
        };
        let footer = html! {<>
            if core_state.players_online >= 2 {
                if core_state.players_on_shard != core_state.players_online {
                    {translate!(t, "{players} in {arena}")}
                    <br/>
                }
                {t.online(core_state.players_online)}
                <br/>
            }
            if props.liveboard && let Some(caveat) = core_state.leaderboard_caveat {
                {match caveat {
                    LeaderboardCaveat::Closing => html_nested!{
                        <span
                            title={translate!(t, "No new players will join; refreshing page will likely move you to new server")}
                            style="pointer-events: all;"
                        >
                            {translate!(t, "Closing")}
                        </span>
                    },
                    LeaderboardCaveat::Unlisted => html_nested!{
                        <span
                            title={translate!(t, "Isolated from leaderboard and most players")}
                            style="pointer-events: all;"
                        >
                            {translate!(t, "Unlisted")}
                        </span>
                    },
                    LeaderboardCaveat::Temporary => html_nested!{
                        <span
                            title={translate!(t, "Scores on party servers are not eligible for the leaderboard")}
                            style="pointer-events: all;"
                        >
                            {translate!(t, "Party")}
                        </span>
                    },
                }}
            }
        </>};

        (items, footer)
    } else {
        let period_id = ctw.setting_cache.leaderboard_period_id;
        let lb = core_state.leaderboard(period_id);
        let items = lb
            .iter()
            .take(count)
            .map(|dto| {
                html_nested! {
                    <tr>
                        <td class="name">{dto.alias.as_str()}</td>
                        <td class="score">{(props.fmt_score)(dto.score)}</td>
                    </tr>
                }
            })
            .chain(
                std::iter::repeat(html_nested! {
                    <tr>
                        <td
                            class="name"
                            style="visibility: hidden;"
                            colspan="2"
                        >{"-"}</td>
                    </tr>
                })
                .take(count.saturating_sub(lb.len())),
            )
            .collect::<Html>();

        let footer = [PeriodId::Daily, PeriodId::Weekly, PeriodId::AllTime]
            .into_iter()
            .filter(|period_id| *period_id != PeriodId::AllTime || props.all_time)
            .map(|period_id| {
                html_nested! {
                    <span
                        onclick={change_period_factory.clone()(period_id)}
                        style={if ctw.setting_cache.leaderboard_period_id == period_id {
                            "text-decoration: underline;"
                        } else {
                            "cursor: pointer; pointer-events: auto;"
                        }}
                    >
                        {t.period_hint(period_id)}
                    </span>
                }
            })
            .intersperse(html!(
                <span style="opacity: 0.6;">{" | "}</span>
            ))
            .collect::<Html>();

        (items, footer)
    };

    html! {
        if ctw.setting_cache.leaderboard && (props.liveboard || !ctw.setting_cache.arena_id.realm_id().is_some_and(|r| r.is_temporary())) {
            <Positioner
                id="leaderboard"
                position={props.position}
                style={"pointer-events: none;".to_owned() + if let Some(style) = &props.style {
                    style.as_str()
                } else { "" }}
                class={classes!(high_contrast_class)}
            >
                <table class={table_css_class}>
                    {items}
                </table>
                <p class={p_css_class}>
                    {footer}
                </p>
            </Positioner>
        }
    }
}

/*
#[cfg(test)]
mod test {
    use crate::overlay::leaderboard::LeaderboardProps;

    #[test]
    fn fmt_abbreviated() {
        assert_eq!(LeaderboardProps::fmt_abbreviated(u32::MAX / 1000 / 1000), "");
        assert_eq!(LeaderboardProps::fmt_abbreviated(u32::MAX / 1000), "");
        assert_eq!(LeaderboardProps::fmt_abbreviated(u32::MAX), "");
    }
}
 */
