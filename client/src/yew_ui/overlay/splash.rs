// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{
    is_mobile, translate, CookieNotice, Ctw, DiscordButton, EngineNexus, Flex, GithubButton,
    GooglePlayButton, InvitationButton, LanguagePicker, NexusButton, Position, Positioner,
    PrivacyLink, RankNumber, RoutableExt, RouteLink, ScopeClaimKey, SettingsIcon, SignInLink,
    SoftbearButton, TermsLink,
};
use std::num::NonZeroU16;
use yew::{html, Html};
use yew_confetti::{Cannon, Confetti, Mode};
use yew_icons::{Icon, IconId};

pub const SPLASH_MARGIN: &str = "0.5rem";

#[derive(Default)]
pub struct SplashLinksProps {
    contrast: bool,
}

impl SplashLinksProps {
    pub fn contrast(mut self, value: bool) -> Self {
        self.contrast = value;
        self
    }
}

pub fn splash_links<R: RoutableExt>(ctw: &Ctw, game: &[R], props: SplashLinksProps) -> Html {
    let t = &ctw.translator;
    html! {
        <Positioner
            id="links"
            position={Position::BottomMiddle{margin: SPLASH_MARGIN}}
            flex={Flex::Row}
            style={props.contrast.then_some("background-color: rgba(0, 0, 0, 0.15); box-shadow: 0px 0.5rem 2rem 0.5rem rgba(0, 0, 0, 0.3);")}
        >
            if !ctw.nexus {
                {game.iter().map(|r| html!{
                    <RouteLink<R> route={r.clone()}>{r.label(t)}</RouteLink<R>>
                }).collect::<Html>()}
                <RouteLink<EngineNexus> route={EngineNexus::Feedback}>{EngineNexus::Feedback.label(t)}</RouteLink<EngineNexus>>
                <PrivacyLink/>
                <TermsLink/>
            }
        </Positioner>
    }
}

#[derive(Default)]
pub struct SplashNexusIconsProps {
    invitation: bool,
}

impl SplashNexusIconsProps {
    pub fn invitation(mut self, value: bool) -> Self {
        self.invitation = value;
        self
    }
}

// These options are duplicated elsewhere, but this is a faster vehicle.
pub fn splash_nexus_icons(ctw: &Ctw, props: SplashNexusIconsProps) -> Html {
    #[cfg(feature = "audio")]
    let volume = html! { <crate::VolumePicker/> };
    #[cfg(not(feature = "audio"))]
    let volume = Html::default();

    #[cfg(feature = "pointer_lock")]
    let show_nexus = !ctw.escaping.is_in_game();
    #[cfg(not(feature = "pointer_lock"))]
    let show_nexus = true;

    html! {
        <Positioner
            id="nexus_icons"
            position={Position::BottomRight{margin: SPLASH_MARGIN}}
            flex={Flex::Row}
        >
            if !ctw.nexus {
                if !ctw.escaping.is_in_game() {
                    <LanguagePicker/>
                    {volume}
                    if ctw.escaping.is_escaping() && props.invitation && !ctw.features.outbound.invitations.is_none() {
                        <InvitationButton/>
                    }
                    if ctw.escaping.is_spawning() {
                        <SettingsIcon/>
                    }
                }
                if !ctw.escaping.is_spawning() && show_nexus {
                    <NexusButton/>
                }
            }
        </Positioner>
    }
}

pub fn splash_sign_in_link(ctw: &Ctw) -> Html {
    let position = Position::BottomLeft {
        margin: SPLASH_MARGIN,
    };
    let t = &ctw.translator;
    let claims = &ctw.state.as_strong().claims;
    let days: Option<(NonZeroU16, bool)> = claims.get(&ScopeClaimKey::streak()).and_then(|c| {
        NonZeroU16::new(c.value.min(u16::MAX as u64) as u16).map(move |days| {
            (
                days,
                c.date_updated.0.get() > (js_sys::Date::now() as u64) - 30 * 1000,
            )
        })
    });
    let rank: Option<Option<RankNumber>> = claims
        .get(&ScopeClaimKey::rank())
        .map(|c| RankNumber::new(c.value.min(u8::MAX as u64) as u8));
    fn mode(days: NonZeroU16) -> Mode {
        let rate = 20 + (days.get() as usize).min(7) * 5;
        let duration = 1.0 + (days.get() as f32).sqrt();
        Mode::delayed_finite_continuous(rate, 1.0, duration)
    }
    let flex_css = stylist::css!(
        r#"
        display: flex;
        flex-direction: row;
        gap: 0.75rem;

        @media (max-width: 800px) {
            flex-direction: column-reverse;
            gap: 0.5rem;
        }
    "#
    );
    html! {
        if ctw.features.cookie_consent && !ctw.setting_cache.cookie_notice_dismissed {
            <CookieNotice {position}/>
        } else if !ctw.nexus {
            <Positioner id="account" {position} class={flex_css}>
                <SignInLink hide_login={is_mobile()}/>
                if let Some((days, recent)) = days {
                    <span
                        title={translate!(t, "Consecutive days played")}
                        style={"position: relative;"}
                    >
                        <Icon
                            width="1.25rem"
                            height="1.25rem"
                            style="vertical-align: middle; margin-right: 0.25rem;"
                            icon_id={IconId::FontAwesomeSolidFire}
                        />
                        {translate!(t, "{days} day streak")}
                        if recent && days.get() > 1 {
                            <Confetti
                                width={256}
                                height={128}
                                style={"position: absolute; width: 400%; aspect-ratio: 2; bottom: 0; left: 50%; transform: translate(-50%, 0);"}
                            >
                                <Cannon x={0.4} y={0.05} velocity={3.1} spread={0.45} mode={mode(days)}/>
                                <Cannon x={0.5} y={0.05} velocity={3.1} spread={0.45} mode={mode(days)}/>
                                <Cannon x={0.6} y={0.05} velocity={3.1} spread={0.45} mode={mode(days)}/>
                            </Confetti>
                        }
                    </span>
                }
                if let Some(rank) = rank {
                    <RouteLink<EngineNexus>
                        route={EngineNexus::Ranks}
                        title={translate!(t, "Rank")}
                        style={"position: relative;"}
                    >
                        <Icon
                            width="1.25rem"
                            height="1.25rem"
                            style="vertical-align: middle; margin-right: 0.25rem;"
                            icon_id={IconId::FontAwesomeSolidAward}
                        />
                        if let Some(rank) = rank {
                            {(ctw.translate_rank_number)(&t, rank)}
                        } else {
                            {translate!(t, "Unranked")}
                        }
                    </RouteLink<EngineNexus>>
                }
            </Positioner>
        }
    }
}

#[derive(Default)]
pub struct SplashSocialMediaProps {
    github: Option<&'static str>,
    google_play: Option<&'static str>,
}

impl SplashSocialMediaProps {
    pub fn github(mut self, github: &'static str) -> Self {
        self.github = Some(github);
        self
    }

    pub fn google_play(mut self, google_play: &'static str) -> Self {
        self.google_play = Some(google_play);
        self
    }
}

pub fn splash_social_media(ctw: &Ctw, props: SplashSocialMediaProps) -> Html {
    html! {
        <Positioner
            id={"social_icons"}
            position={Position::TopLeft{margin: SPLASH_MARGIN}}
            flex={Flex::Row}
        >
            if !ctw.nexus {
                // Internally hidden if social media feature off.
                <DiscordButton circle={true}/>
                if let Some(google_play_url) = props.google_play {
                    // Internally hidden if app store feature off.
                    <GooglePlayButton {google_play_url} circle={true}/>
                }
                if let Some(repository_url) = props.github {
                    // Internally hidden if social media feature off.
                    <GithubButton {repository_url} circle={true}/>
                }
                <SoftbearButton/>
            }
        </Positioner>
    }
}
