// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::{PlasmaActlet, ServerActor};
use crate::rate_limiter::RateLimiter;
use crate::service::{ArenaService, LiveboardRepo, Score};
use crate::{LeaderboardScoreDto, LeaderboardUpdate, PeriodId, PlasmaRequestV1, PlayerAlias};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

// TODO: was pub(crate)
#[derive(Debug, Default)]
pub struct PlayerLeaderboardData {
    pub(crate) high_score: u32,
}

impl PlayerLeaderboardData {
    pub(crate) fn update_score(&mut self, score: Score) {
        if let Score::Some(score) = score {
            self.high_score = self.high_score.max(score);
        }
    }
}

/// Manages updating, saving, and loading leaderboards.
pub struct LeaderboardRepo<G: ArenaService> {
    /// Stores cached leaderboards from database and whether they were changed.
    leaderboards: [(Arc<[LeaderboardScoreDto]>, bool); std::mem::variant_count::<PeriodId>()],
    /// Scores that should be committed to database.
    pending: HashMap<PlayerAlias, u32>,
    take_pending_rate_limit: RateLimiter,
    _spooky: PhantomData<G>,
}

impl<G: ArenaService> Default for LeaderboardRepo<G> {
    fn default() -> Self {
        Self {
            leaderboards: [
                (Vec::new().into(), false),
                (Vec::new().into(), false),
                (Vec::new().into(), false),
            ],
            pending: HashMap::new(),
            take_pending_rate_limit: RateLimiter::new(Duration::from_secs(60), 0),
            _spooky: PhantomData,
        }
    }
}

impl<G: ArenaService> LeaderboardRepo<G> {
    /// Gets a cached leaderboard.
    pub fn get(&self, period_id: PeriodId) -> &Arc<[LeaderboardScoreDto]> {
        &self.leaderboards[period_id as usize].0
    }

    /// Leaderboard relies on an external source of data, such as a database.
    pub fn put_leaderboard(
        &mut self,
        period_id: PeriodId,
        leaderboard: Box<[LeaderboardScoreDto]>,
    ) {
        let leaderboard: Arc<[LeaderboardScoreDto]> = Vec::from(leaderboard).into();
        if &leaderboard != self.get(period_id) {
            self.leaderboards[period_id as usize] = (leaderboard, true);
        }
    }

    /// Computes minimum score to earn a place on the given leaderboard.
    fn minimum_score(&self, period_id: PeriodId) -> u32 {
        self.get(period_id)
            .get(G::LEADERBOARD_SIZE - 1)
            .map(|dto| dto.score)
            .unwrap_or(1)
    }

    /// Process liveboard scores to potentially be added to the leaderboard.
    pub(crate) fn update(&mut self, liveboard: &LiveboardRepo<G>, plasma: &PlasmaActlet) {
        if !liveboard.dirty {
            return;
        }
        let liveboard_items = liveboard.get();

        // Must be sorted in reverse.
        debug_assert!(liveboard_items.is_sorted_by_key(|dto| u32::MAX - dto.score));

        if (cfg!(not(debug_assertions)) && liveboard.player_count < G::LEADERBOARD_MIN_PLAYERS)
            || (plasma.role.is_unlisted() || plasma.role.is_closing())
        {
            return;
        }

        for dto in liveboard_items.iter() {
            if PeriodId::iter().all(|period_id| dto.score < self.minimum_score(period_id)) {
                // Sorted, so this iteration is not going to produce any more sufficient scores.
                break;
            }

            // TODO
            /*
            if dto.player_id.is_bot() {
                // Bots are never on the leaderboard, regardless of whether they are on the liveboard.
                continue;
            }
            */

            let entry = self.pending.entry(dto.alias).or_insert(0);
            *entry = dto.score.max(*entry);
        }
    }

    /// Returns scores pending database commit, draining them in the process. Rate limited.
    pub fn take_pending(&mut self) -> Option<Box<[LeaderboardScoreDto]>> {
        if self.pending.is_empty() || self.take_pending_rate_limit.should_limit_rate() {
            None
        } else {
            Some(
                self.pending
                    .drain()
                    .map(|(alias, score)| LeaderboardScoreDto { alias, score })
                    .collect(),
            )
        }
    }

    pub fn update_to_plasma(infrastructure: &mut ServerActor<G>) {
        for (realm_id, context_realm) in infrastructure.realms.realms_mut() {
            if let Some(scores) = context_realm.realm_context.leaderboard.take_pending() {
                infrastructure
                    .plasma
                    .do_request(PlasmaRequestV1::UpdateLeaderboards { realm_id, scores });
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (PeriodId, &Arc<[LeaderboardScoreDto]>)> {
        self.leaderboards
            .iter()
            .enumerate()
            .map(|(i, (leaderboard, _))| (PeriodId::from(i), leaderboard))
    }

    /// Reads off changed leaderboards, *without* the changed flag in the process.
    pub fn deltas_nondestructive(
        &self,
    ) -> impl Iterator<Item = (PeriodId, &Arc<[LeaderboardScoreDto]>)> {
        self.leaderboards
            .iter()
            .enumerate()
            .filter_map(|(i, (leaderboard, changed))| {
                if *changed {
                    Some((PeriodId::from(i), leaderboard))
                } else {
                    None
                }
            })
    }

    /// Clear all the delta flags (such as if clients have been updated).
    pub(crate) fn clear_deltas(&mut self) {
        for (_, changed) in self.leaderboards.iter_mut() {
            *changed = false;
        }
    }

    /// Gets leaderboard for new players.
    pub(crate) fn initializers(&self) -> impl Iterator<Item = LeaderboardUpdate> + '_ {
        self.iter().filter_map(|(period_id, leaderboard)| {
            if leaderboard.is_empty() {
                None
            } else {
                Some(LeaderboardUpdate::Updated(
                    period_id,
                    Arc::clone(leaderboard),
                ))
            }
        })
    }
}
