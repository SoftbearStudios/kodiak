// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::sprite_sheet::{SpriteGeo, SpriteSheet};
use crate::{
    translate, use_client_request_callback, use_core_state, use_ctw, use_translator, ClientRequest,
    NexusDialog, RankNumber, ScopeClaimKey, Sprite, SpriteSheetDetails,
};
use std::sync::LazyLock;
use strum::IntoEnumIterator;
use stylist::css;
use yew::{function_component, html, html_nested, Html, InputEvent};

#[function_component(RanksDialog)]
pub fn ranks_dialog() -> Html {
    let ctw = use_ctw();
    let t = use_translator();

    let core_state = use_core_state();
    let my_rank: Option<Option<RankNumber>> = core_state.rank();
    let announcement_preference = core_state
        .claims
        .get(&ScopeClaimKey::announcement_preference())
        .map(|v| v.value != 0)
        .unwrap_or(true);

    static SPRITE_SHEET: LazyLock<SpriteSheetDetails> = LazyLock::new(|| {
        let image_src = include_str!("../../../../assets/sprites/ranks.webp.url");
        const RES: u32 = 64;
        let sprite_sheet = SpriteSheet {
            width: RES,
            height: RES * 6,
            sprites: (0..6)
                .map(|i| {
                    (
                        (i + 1).to_string(),
                        SpriteGeo {
                            x: 0,
                            y: RES * i,
                            width: RES,
                            height: RES,
                        },
                    )
                })
                .collect(),
            animations: Default::default(),
        };
        SpriteSheetDetails {
            sprite_sheet,
            image_src,
        }
    });

    let container_style = css!(
        r#"
        display: grid;
        grid-template-columns: repeat(1, 1fr);
        height: 100%;
    "#
    );

    let rank_style = css!(
        r#"
        display: flex;
        flex-direction: row;
        gap: 1rem;
        background-color: #00000033;
        border-radius: 0.5rem;
        margin: 0.5rem;
        padding: 0.5rem;
    "#
    );

    let client_request_callback = use_client_request_callback();

    html! {
        <NexusDialog title={translate!(t, "Ranks")}>
            <p>{translate!(t, "You can earn ranks by playing and getting better at the game.")}</p>
            <p>{translate!(t, "It is possible to reach the highest rank by playing normally, but defeating players with considerably more score than you is the fastest way to rank up.")}</p>
            if ctw.features.outbound.accounts.is_some() && ctw.setting_cache.user_name.is_none() && ctw.setting_cache.nick_name.is_none() {
                <p>{translate!(t, "Clearing your cookies will reset your rank, unless you make an account.")}</p>
            }
            <div class={container_style}>
                {RankNumber::iter().map(|rank| {
                    let benefits = (ctw.translate_rank_benefits)(&t, rank);
                    let mut benefits = benefits
                        .into_iter()
                        .map(|s| html!{{s}})
                        .chain(match rank {
                            RankNumber::Rank1 => vec![html!{{translate!(t, "Rank visible in public profile")}}],
                            RankNumber::Rank2 => vec![],
                            RankNumber::Rank3 => vec![],
                            RankNumber::Rank4 => {
                                let mut ret = vec![];
                                if ctw.features.chat {
                                    let oninput = client_request_callback.reform(move |_: InputEvent| {
                                        ClientRequest::AnnouncementPreference(!announcement_preference)
                                    });
                                    ret.push(html!{
                                        <label>
                                            <input
                                                type="checkbox"
                                                checked={announcement_preference}
                                                style="vertical-align: middle;"
                                                {oninput}
                                                disabled={my_rank.flatten() < Some(rank)}
                                            />
                                            {translate!(t, "Join announced in chat")}
                                        </label>
                                    });
                                }
                                ret
                            }
                            RankNumber::Rank5 => vec![],
                            RankNumber::Rank6 => vec![],
                        })
                        .intersperse(html!{{" | "}})
                        .peekable();
                    html_nested!{
                        <div
                            class={rank_style.clone()}
                            style={(my_rank == Some(Some(rank))).then_some("border: 2px solid #ffffff55;")}
                        >
                            <Sprite sheet={&*SPRITE_SHEET} sprite={rank.get().to_string()}/>
                            <div>
                                <h3>{(ctw.translate_rank_number)(&t, rank)}</h3>
                                <p>
                                    if benefits.peek().is_none() {
                                        {translate!(t, "Coming soon...")}
                                    } else {
                                        {benefits.collect::<Html>()}
                                    }
                                </p>
                            </div>
                        </div>
                    }
                }).collect::<Html>()}
            </div>
        </NexusDialog>
    }
}
