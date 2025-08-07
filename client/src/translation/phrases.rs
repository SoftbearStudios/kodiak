// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{translate, Translator};
use crate::{
    markdown, translated_text, DiscordButton, FatalError, GameConstants, MarkdownOptions, PeriodId,
    RegionId, SettingCategory, CONTACT_EMAIL,
};
use yew::{html, Html};

// Find: translate!\((.*), "(.*)", "(.*)"\)
// Replace: translate!($1, "$3")

impl Translator {
    pub fn mouse_controls(&self) -> String {
        translate!(self, "Mouse Controls")
    }

    pub fn keyboard_controls(&self) -> String {
        translate!(self, "Keyboard Controls")
    }

    pub fn watch_ad_to_unlock(&self) -> String {
        translate!(self, "Watch ad to unlock.")
    }

    pub fn earn_rank_to_unlock(&self, rank: String) -> String {
        translate!(self, "Earn {rank} rank to unlock.")
    }

    /// How to display the language in the settings dropdown.
    pub fn label(&self) -> String {
        // "label" must match game server translation actlet.
        translate!(self, "English")
    }

    pub fn chat_label(&self) -> String {
        translate!(self, "Chat")
    }

    pub fn chat_radio_label(&self) -> String {
        translate!(self, "Radio")
    }

    pub fn chat_send_message_hint(&self) -> String {
        translate!(self, "Press Enter to send")
    }

    pub fn chat_send_team_message_hint(&self) -> String {
        translate!(
            self,
            "chat_send_team_message_hint",
            "Press Enter to send, or Shift+Enter to send to team only"
        )
    }

    pub fn chat_send_message_placeholder(&self) -> String {
        translate!(self, "Message")
    }

    //  pub fn chat_mute_label(&self) -> String {
    //      translate!(self, "Mute")
    //  }

    //  pub fn chat_report_label(&self) -> String {
    //      translate!(self, "Report")
    //  }

    pub fn period_all_time_hint(&self) -> String {
        translate!(self, "All")
    }

    pub fn period_weekly_hint(&self) -> String {
        translate!(self, "Week")
    }

    pub fn period_daily_hint(&self) -> String {
        translate!(self, "Day")
    }

    pub fn period_hint(&self, period_id: PeriodId) -> String {
        match period_id {
            PeriodId::AllTime => self.period_all_time_hint(),
            PeriodId::Daily => self.period_daily_hint(),
            PeriodId::Weekly => self.period_weekly_hint(),
        }
    }

    pub fn team_label(&self) -> String {
        translate!(self, "Team")
    }

    pub fn team_accept_hint(&self) -> String {
        translate!(self, "Accept")
    }

    pub fn team_accept_full_hint(&self) -> String {
        translate!(self, "Team full")
    }

    pub fn team_create_hint(&self) -> String {
        translate!(self, "Create")
    }

    pub fn team_deny_hint(&self) -> String {
        translate!(self, "Deny")
    }

    pub fn team_kick_hint(&self) -> String {
        translate!(self, "Kick")
    }

    pub fn team_leave_hint(&self) -> String {
        translate!(self, "Leave")
    }

    pub fn team_name_placeholder(&self) -> String {
        translate!(self, "New team")
    }

    pub fn team_request_hint(&self) -> String {
        translate!(self, "Request Join")
    }

    pub fn online(&self, players: u32) -> String {
        translate!(self, "{players} online")
    }

    pub fn level(&self, level: u32) -> String {
        translate!(self, "Level {level}")
    }

    pub fn upgrade_label(&self) -> String {
        translate!(self, "Upgrade")
    }

    pub fn upgrade_to_label(&self, upgrade: &str) -> String {
        translate!(self, "Upgrade to {upgrade}")
    }

    pub fn downgrade_to_label(&self, downgrade: &str) -> String {
        translate!(self, "Downgrade to {downgrade}")
    }

    pub fn upgrade_to_level_label(&self, level: u32) -> String {
        translate!(self, "Upgrade to level {level}")
    }

    pub fn upgrade_to_level_progress(&self, percent: u8, level: u32) -> String {
        translate!(
            self,
            "upgrade_to_level_progress",
            "{percent}% to level {level}"
        )
    }

    pub fn respawn_as_level_label(&self, level: u32) -> String {
        translate!(self, "Respawn as level {level}")
    }

    pub fn splash_screen_alias_placeholder(&self) -> String {
        translate!(self, "Nickname")
    }

    // Referenced multiple times.
    pub fn invitation_label(&self) -> String {
        translate!(self, "Copy Invite")
    }

    // Referenced multiple times.
    pub fn invitation_copied_label(&self) -> String {
        translate!(self, "Invite copied!")
    }

    pub fn alert_dismiss(&self) -> String {
        translate!(self, "Dismiss")
    }

    pub fn ok_label(&self) -> String {
        translate!(self, "OK")
    }

    pub fn cancel_label(&self) -> String {
        translate!(self, "Cancel")
    }

    pub fn close_label(&self) -> String {
        translate!(self, "Close")
    }

    pub fn point(&self) -> String {
        translate!(self, "point")
    }

    pub fn points(&self) -> String {
        translate!(self, "points")
    }

    pub fn score(&self, score: u32) -> String {
        // Good enough for simple plural vs. singular dichotomy, but can be overridden if needed.
        let suffix = match score {
            1 => self.point(),
            _ => self.points(),
        };
        format!("{} {}", score, suffix)
    }

    // Referenced in games.
    pub fn about_hint(&self) -> String {
        translate!(self, "About")
    }

    // Referenced in games.
    pub fn about_title(&self, game_constants: &'static GameConstants) -> String {
        let game_name = game_constants.name;
        translate!(self, "About {game_name}")
    }

    // Referenced in games.
    pub fn about_contact(&self) -> Html {
        let md = translated_text!(self, "about_contact_md");

        markdown(
            &md,
            &MarkdownOptions {
                components: Box::new(|href, _| match href {
                    "Discord" => Some(html! {
                        <DiscordButton size={"1.5rem"}/>
                    }),
                    "email" => Some(html! {
                        <a href={format!("mailto:{}", CONTACT_EMAIL)}>{CONTACT_EMAIL}</a>
                    }),
                    _ => None,
                }),
                ..Default::default()
            },
        )
    }

    // Referenced in games.
    pub fn help_hint(&self) -> String {
        translate!(self, "Help")
    }

    pub fn help_title(&self, game_constants: &'static GameConstants) -> String {
        let game_name = game_constants.name;
        translate!(self, "{game_name} Help Guide")
    }

    pub fn resume_hint(&self) -> String {
        translate!(self, "Resume")
    }

    pub fn quit_hint(&self) -> String {
        translate!(self, "Quit")
    }

    pub fn learn_more_label(&self) -> String {
        translate!(self, "Learn more")
    }

    // Referenced multiple times.
    pub fn settings_title(&self) -> String {
        translate!(self, "Settings")
    }

    pub fn changelog_hint(&self) -> String {
        translate!(self, "Changelog")
    }

    pub fn changelog_title(&self, game_constants: &'static GameConstants) -> String {
        let game_name = game_constants.name;
        translate!(self, "{game_name} Updates")
    }

    pub fn find_game_title(&self) -> String {
        translate!(self, "Find Game")
    }

    pub fn profile_label(&self) -> String {
        translate!(self, "Profile")
    }

    pub fn feedback_label(&self) -> String {
        translate!(self, "Feedback")
    }

    pub fn abbreviated_region_id(&self, region_id: RegionId) -> String {
        // TODO: different languages may need different abbreviations. Use phrase id.
        match region_id {
            RegionId::NorthAmerica => translate!(self, "N. America"),
            RegionId::SouthAmerica => translate!(self, "S. America"),
            _ => self.region_id(region_id),
        }
    }

    pub fn region_id(&self, region_id: RegionId) -> String {
        match region_id {
            RegionId::Africa => translate!(self, "Africa"),
            RegionId::Asia => translate!(self, "Asia"),
            RegionId::Europe => translate!(self, "Europe"),
            RegionId::NorthAmerica => translate!(self, "North America"),
            RegionId::Oceania => translate!(self, "Oceania"),
            RegionId::SouthAmerica => translate!(self, "South America"),
        }
    }

    pub fn setting_category(&self, setting_category: SettingCategory) -> String {
        #[cfg_attr(not(feature = "audio"), allow(unused))]
        let audio = || translate!(self, "Audio");
        match setting_category {
            SettingCategory::General => translate!(self, "General"),
            SettingCategory::Graphics => translate!(self, "Graphics"),
            #[cfg(feature = "audio")]
            SettingCategory::Audio => audio(),
            SettingCategory::Privacy => translate!(self, "Privacy"),
        }
    }

    /// Can't inline this because it would be compiled out if zoom was disabled.
    pub fn zoom_in(&self) -> String {
        translate!(self, "Zoom In")
    }

    /// Can't inline this because it would be compiled out if zoom was disabled.
    pub fn zoom_out(&self) -> String {
        translate!(self, "Zoom Out")
    }

    pub fn fatal_error(&self, error: FatalError) -> String {
        match error {
            FatalError::WebGl => translate!(self, "WebGL unsupported"),
            FatalError::WebGl2 => translate!(self, "WebGL2 unsupported"),
        }
    }
}
