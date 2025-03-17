// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::ClientMetricData;
use crate::bitcode::{self, *};
use crate::{
    ArenaId, ClientActivity, NavigationMetricsDto, NonZeroUnixMillis, QuestEvent, QuestEventDto,
    QuestSampleDto, QuestState, ServerId, UnixTime,
};

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientQuestData {
    date_created: NonZeroUnixMillis,
    server_id: ServerId,
    arena_id: ArenaId,
    navigation: NavigationMetricsDto,
    events: Vec<QuestEventDto>,
    highest_score: u32,
    last_fps: Option<f32>,
    last_rtt: Option<u16>,
    last_activity: ClientActivity,
    last_tutorial_step: u8,
    last_closing: bool,
    traces: u8,
    pub(crate) quit: bool,
}

impl ClientQuestData {
    pub fn new(
        server_id: ServerId,
        arena_id: ArenaId,
        navigation: NavigationMetricsDto,
        last_quest_date_created: &mut NonZeroUnixMillis,
    ) -> Self {
        let date_created = NonZeroUnixMillis::now().max(last_quest_date_created.add_millis(1));
        *last_quest_date_created = date_created;
        Self {
            date_created,
            server_id,
            arena_id,
            events: Default::default(),
            highest_score: 0,
            last_fps: None,
            last_rtt: None,
            last_activity: Default::default(),
            last_tutorial_step: 0,
            last_closing: false,
            traces: 0,
            navigation,
            quit: false,
        }
    }

    pub fn update_closing(&mut self, closing: bool) {
        if closing == self.last_closing {
            return;
        }
        self.last_closing = closing;
        self.push(QuestEvent::Closing { closing });
    }

    pub fn update_score(&mut self, score: u32) {
        if score == 0 {
            return;
        }
        let log10 = score.ilog10();
        if self.highest_score > 0 && log10 <= self.highest_score.ilog10() {
            return;
        }
        self.highest_score = score;
        self.push(QuestEvent::Score {
            score: 10u32.pow(log10),
        })
    }

    pub fn push(&mut self, ev: QuestEvent) {
        if self.events.len() >= 256 {
            return;
        }
        match ev {
            QuestEvent::Fps { fps } => {
                if let Some(last_fps) = self.last_fps
                    && (fps - last_fps).abs() < 10.0
                {
                    return;
                }
                self.last_fps = Some(fps);
            }
            QuestEvent::Rtt { rtt } => {
                if let Some(last_rtt) = self.last_rtt
                    && rtt.abs_diff(last_rtt) < 20
                {
                    return;
                }
                self.last_rtt = Some(rtt);
            }
            QuestEvent::Activity { activity } => {
                if activity == self.last_activity {
                    // Deduplicate hidden events.
                    return;
                }
                self.last_activity = activity;
            }
            QuestEvent::Tutorial { step } => {
                if step <= self.last_tutorial_step {
                    return;
                }
                self.last_tutorial_step = step;
            }
            QuestEvent::State {
                state: QuestState::Spawning {},
            } => {
                self.quit = false;
            }
            QuestEvent::Arena { .. } => {
                self.last_closing = false;
            }
            QuestEvent::Trace { ref message } => {
                if self.traces >= 3 || message.len() > QuestEvent::TRACE_LIMIT {
                    return;
                }
                self.traces += 1;
            }
            _ => {}
        }
        self.events.push(QuestEventDto {
            t: NonZeroUnixMillis::now().millis_since(self.date_created),
            e: ev,
        })
    }

    pub fn sample(self, metrics: &ClientMetricData) -> QuestSampleDto {
        QuestSampleDto {
            date_created: self.date_created,
            date_visitor_created: metrics.date_created,
            cohort_id: metrics.cohort_id,
            language_id: metrics.language_id,
            referrer: metrics.referrer,
            region_id: metrics.region_id,
            user_agent_id: metrics.user_agent_id,
            lifecycle_id: Some(metrics.lifecycle),
            server_id: self.server_id,
            arena_id: self.arena_id,
            navigation: self.navigation,
            events: self.events.into(),
        }
    }
}
