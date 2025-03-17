// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::browser::BrowserStorages;
use crate::js_hooks::window;
use crate::{
    settings_prerequisites, translate, ArenaQuery, CohortId, DeepConnect, LanguageId,
    NonZeroUnixMillis, PeriodId, PlayerAlias, ServerId, ServerKind, SessionId, SessionToken,
    Settings, Translator,
};
use kodiak_common::rand::seq::SliceRandom;
use kodiak_common::rand::{random, thread_rng};
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::str::FromStr;
use strum_macros::IntoStaticStr;

/// Settings backed by local storage.
pub trait LocalSettings: Sized {
    /// Loads all settings from local storage.
    fn load(l: &BrowserStorages, default: Self) -> Self;

    /// Storage keys for preferences.
    fn preferences() -> &'static [&'static str];
    /// Storage keys for statistics.
    fn statistics() -> &'static [&'static str];
    /// Storage keys for posting.
    fn synchronize(&self, known: &HashMap<String, String>, browser_storages: &mut BrowserStorages);

    /// Renders GUI widgets for certain settings.
    fn display(
        &self,
        t: &Translator,
        checkbox: impl FnMut(SettingCategory, String, bool, fn(&mut Self, bool, &mut BrowserStorages)),
        dropdown: impl FnMut(
            SettingCategory,
            String,
            &'static str,
            fn(usize) -> Option<(&'static str, &'static str)>,
            fn(&mut Self, &str, &mut BrowserStorages),
        ),
        slider: impl FnMut(
            SettingCategory,
            String,
            f32,
            RangeInclusive<f32>,
            fn(&mut Self, f32, &mut BrowserStorages),
        ),
    );
}

#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Debug, Default, IntoStaticStr)]
pub enum SettingCategory {
    #[default]
    General,
    #[cfg(feature = "audio")]
    Audio,
    Graphics,
    Privacy,
}

// Useful if you don't want settings.
impl LocalSettings for () {
    fn load(_: &BrowserStorages, _: Self) -> Self {}

    fn preferences() -> &'static [&'static str] {
        &[]
    }

    fn statistics() -> &'static [&'static str] {
        &[]
    }

    fn synchronize(
        &self,
        _known: &HashMap<String, String>,
        _browser_storages: &mut BrowserStorages,
    ) {
    }

    fn display(
        &self,
        _: &Translator,
        _: impl FnMut(SettingCategory, String, bool, fn(&mut Self, bool, &mut BrowserStorages)),
        _: impl FnMut(
            SettingCategory,
            String,
            &'static str,
            fn(usize) -> Option<(&'static str, &'static str)>,
            fn(&mut Self, &str, &mut BrowserStorages),
        ),
        _: impl FnMut(
            SettingCategory,
            String,
            f32,
            RangeInclusive<f32>,
            fn(&mut Self, f32, &mut BrowserStorages),
        ),
    ) {
    }
}

/// Settings of the infrastructure, common to all games.
#[derive(Clone, PartialEq, Settings)]
pub struct CommonSettings {
    /// Alias preference.
    #[setting(preference, optional)]
    pub alias: Option<PlayerAlias>,
    /// Random Guest name if there is no alias.
    #[setting(preference, volatile)]
    pub random_guest_alias: PlayerAlias,
    /// Language preference.
    #[setting(preference, post)]
    pub language: LanguageId,
    /// Volume preference (0 to 1).
    #[cfg(feature = "audio")]
    #[setting(preference, range = "0.0..1.0", finite, post)]
    pub volume: f32,
    /// Music preference.
    #[cfg(feature = "music")]
    #[setting(preference, checkbox = "Audio/Music", post)]
    pub music: bool,
    /// Last [`CohortId`].
    #[setting(statistic)]
    pub cohort_id: CohortId,
    /// Last-used/chosen [`ServerId`].
    #[setting(optional, volatile)]
    pub server_id: Option<ServerId>,
    /// Last-used/chosen [`ArenaId`].
    #[setting(volatile)]
    pub arena_id: ArenaQuery,
    /// Not manually set by the player.
    #[setting(statistic, optional)]
    pub date_created: Option<NonZeroUnixMillis>,
    /// Not manually set by the player.
    #[setting(optional)]
    pub session_id: Option<SessionId>,
    /// Not manually set by the player.
    #[setting(optional, no_store)]
    pub session_token: Option<SessionToken>,
    /// Not manually set by the player.
    #[setting(optional)]
    pub nick_name: Option<String>,
    /// Not manually set by the player.
    #[setting(optional)]
    pub user_name: Option<String>,
    #[setting(volatile)]
    pub store_enabled: bool,
    /// Pending chat message.
    #[setting(volatile)]
    pub chat_message: String,
    /// Whether to add a contrasting border behind UI elements.
    #[setting(preference, checkbox = "High contrast", post)]
    #[cfg(feature = "high_contrast_setting")]
    pub high_contrast: bool,
    /// Whether chat menu is open.
    #[setting(preference, checkbox = "Chat", post)]
    pub chat: bool,
    /// Whether leaderboard menu is open.
    #[setting(preference, checkbox = "Leaderboard", post)]
    pub leaderboard: bool,
    #[setting(volatile)]
    pub leaderboard_period_id: PeriodId,
    #[setting(post)]
    pub cookie_notice_dismissed: bool,
    #[setting(checkbox = "Privacy/Allow preference cookies", post)]
    pub preference_cookies: bool,
    #[setting(checkbox = "Privacy/Allow statistics cookies", post)]
    pub statistic_cookies: bool,
    //#[setting(checkbox = "Allow marketing cookies")]
    //pub marketing_cookies: bool,
    #[cfg(feature = "pointer_lock")]
    #[setting(
        preference,
        range = "0.5..1.5",
        finite,
        slider = "Mouse sensitivity",
        post
    )]
    pub mouse_sensitivity: f32,
}

impl Default for CommonSettings {
    fn default() -> Self {
        #[cfg(not(feature = "music"))]
        #[allow(unreachable_code)]
        if false {
            // Ensure this translation always happens regardless of feature flags.
            #[allow(unused)]
            let t: Translator = unimplemented!();
            let _ = translate!(t, "Music");
        }

        #[cfg(not(feature = "pointer_lock"))]
        #[allow(unreachable_code)]
        if false {
            // Ensure this translation always happens regardless of feature flags.
            #[allow(unused)]
            let t: Translator = unimplemented!();
            let _ = translate!(t, "Mouse sensitivity");
        }

        let deep_connect = crate::deep_connect();
        Self {
            alias: None,
            random_guest_alias: random_guest(),
            language: window()
                .navigator()
                .language()
                .as_ref()
                .and_then(|l| l.get(0..2))
                .and_then(|l| LanguageId::from_str(l).ok())
                .unwrap_or_default(),
            #[cfg(feature = "audio")]
            volume: 0.5,
            #[cfg(feature = "music")]
            music: true,
            cohort_id: random(),
            server_id: deep_connect
                .and_then(|dc| dc.invitation_id())
                .map(|i| i.server_number())
                .map(|number| ServerId {
                    // TODO: use Local if url is localhost.
                    kind: ServerKind::Cloud,
                    number,
                }),
            arena_id: deep_connect
                .and_then(|dc| {
                    Some(match dc {
                        DeepConnect::Realm(realm_id) => ArenaQuery::AnyInstance(realm_id, None),
                        DeepConnect::Invitation(invitation_id) => {
                            ArenaQuery::Invitation(invitation_id)
                        }
                    })
                })
                .unwrap_or_default(),
            session_id: None,
            session_token: None,
            nick_name: None,
            user_name: None,
            store_enabled: false,
            date_created: None,
            chat_message: String::new(),
            #[cfg(feature = "high_contrast_setting")]
            high_contrast: false,
            chat: true,
            leaderboard: true,
            leaderboard_period_id: PeriodId::Daily,
            cookie_notice_dismissed: false,
            preference_cookies: false,
            statistic_cookies: false,
            #[cfg(feature = "pointer_lock")]
            mouse_sensitivity: 1.0,
        }
    }
}

fn random_guest() -> PlayerAlias {
    let options = [
        "AcidGuest",
        "BestGuest",
        "BestGuestern",
        "BirdsGuest",
        "BlessedGuest",
        "CourtGuester",
        "Diguest",
        "Disguest",
        "Diguested",
        "EnderGuest",
        "Guest",
        "Guestament",
        "Guestavo",
        "Guestbound",
        "GuestChoice",
        "GuestControl",
        "GuestrnUnion",
        "GuestEver",
        "GuestHouse",
        "Guesticulate",
        "Guestify",
        "Guestimate",
        "GuestInPeace",
        "Guestival",
        "Guestnut",
        "GuestOfWind",
        "GuestOption",
        "Guesture",
        "GuestWestern",
        "Guesty",
        "Inguest",
        "Inguested",
        "LemonGuest",
        "Lifeguest",
        "LimeGuest",
        "Maniguest",
        "MetaGuest",
        "Proguest",
        "SafetyGuest",
        "Southguest",
        "Sugguest",
        "TakeAGuest",
        "TheGuest",
        "WildGuest",
    ];
    PlayerAlias::new_unsanitized(options.choose(&mut thread_rng()).unwrap())
}
