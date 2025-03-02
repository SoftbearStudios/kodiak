// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{ArenaService, ContinuousExtremaMetricAccumulator};
use log::error;
use simple_server_status::SimpleServerStatus;
use std::mem;
use std::time::{Duration, Instant};

/// Keeps track of the "health" of the server.
pub struct Health {
    system: SimpleServerStatus,
    last: Instant,
    /// Cached CPU fraction.
    cpu: f32,
    cpu_steal: f32,
    /// Cached RAM fraction.
    ram: f32,
    swap: f32,
    /// Cached tick completion fraction.
    missed_ticks: f32,
    missed_ticks_start: Instant,
    /// Ticks completed since `last`.
    ticks_for_missed_ticks: usize,
    /// Seconds per tick.
    spt: ContinuousExtremaMetricAccumulator,
    /// Ticks per second.
    tps: ContinuousExtremaMetricAccumulator,
    /// Ticks in current TPS measurement period.
    ticks: usize,
    /// Start of TPS measurement.
    tps_start: Instant,
}

impl Health {
    /// How long to cache data for (getting data is relatively expensive).
    const CACHE: Duration = Duration::from_secs(30);

    /// Get (possibly cached) cpu usage from 0 to 1.
    pub fn cpu(&mut self) -> f32 {
        self.refresh_if_necessary();
        self.cpu
    }

    /// Get (possibly cached) cpu steal from 0 to 1.
    pub fn cpu_steal(&mut self) -> f32 {
        self.refresh_if_necessary();
        self.cpu_steal
    }

    /// Get (possibly cached) ram usage from 0 to 1.
    pub fn ram(&mut self) -> f32 {
        self.refresh_if_necessary();
        self.ram
    }

    /// Get (possibly cached) tick miss rate from 0 to 1.
    pub fn missed_ticks(&mut self) -> f32 {
        self.refresh_if_necessary();
        self.missed_ticks
    }

    /// Get (possibly cached) bytes/second received.
    pub fn bandwidth_rx(&mut self) -> u64 {
        self.refresh_if_necessary();
        self.system.net_reception_bandwidth().unwrap_or(0)
    }

    /// Get (possibly cached) bytes/second transmitted.
    pub fn bandwidth_tx(&mut self) -> u64 {
        self.refresh_if_necessary();
        self.system.net_transmission_bandwidth().unwrap_or(0)
    }

    /// Get (possibly cached) TCP/UDP connection/socket count.
    pub fn connections(&mut self) -> usize {
        self.refresh_if_necessary();
        self.system.conntrack_sessions().unwrap_or(0)
    }

    /// Call to get average TPS over a large interval.
    pub fn take_tps(&mut self) -> ContinuousExtremaMetricAccumulator {
        mem::take(&mut self.tps)
    }

    /// Take seconds-per-tick measurements.
    pub fn take_spt(&mut self) -> ContinuousExtremaMetricAccumulator {
        mem::take(&mut self.spt)
    }

    /// Call every update a.k.a. tick.
    pub fn record_tick<G: ArenaService>(&mut self, now: Instant, elapsed: f32) {
        self.ticks_for_missed_ticks += 1;
        self.spt.push(elapsed.clamp(0.0, 10.0));

        let tps_elapsed = now.duration_since(self.tps_start);
        if tps_elapsed >= Duration::from_secs_f32(1.0 - G::TICK_PERIOD_SECS * 0.5) {
            if tps_elapsed >= Duration::from_secs(1) {
                self.ticks = self.ticks.saturating_add(1);
                self.tps.push(self.ticks as f32);
                self.ticks = 0;
            } else {
                self.tps.push(self.ticks as f32);
                self.ticks = 1;
            }

            self.tps_start = now;
        } else {
            self.ticks = self.ticks.saturating_add(1);
        }

        let missed_ticks_elapsed = now.duration_since(self.missed_ticks_start);
        if missed_ticks_elapsed > Duration::from_secs(30) {
            let scheduled_ticks = missed_ticks_elapsed.as_secs_f32() * (1.0 / G::TICK_PERIOD_SECS);
            self.missed_ticks =
                (scheduled_ticks - self.ticks_for_missed_ticks as f32).max(0.0) / scheduled_ticks;
            self.ticks_for_missed_ticks = 0;
            self.missed_ticks_start = now;
        }
    }

    fn refresh_if_necessary(&mut self) {
        if self.last.elapsed() <= Self::CACHE {
            return;
        }
        self.last = Instant::now();
        // Health may fail on local system due to lack of conntrack.
        if let Err(e) = self.system.update()
            && cfg!(not(debug_assertions))
        {
            error!("error updating health: {:?}", e);
        }

        self.cpu = self.system.cpu_usage().unwrap_or(0.0);
        self.cpu_steal = self.system.cpu_stolen_usage().unwrap_or(0.0);
        self.ram = self.system.ram_usage().unwrap_or(0.0);
        self.swap = self.system.ram_swap_usage().unwrap_or(0.0);
    }
}

impl Default for Health {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            system: SimpleServerStatus::default(),
            last: now - Self::CACHE * 2,
            cpu: 0.0,
            cpu_steal: 0.0,
            ram: 0.0,
            swap: 0.0,
            missed_ticks: 0.0,
            missed_ticks_start: now,
            ticks_for_missed_ticks: 0,
            ticks: 0,
            spt: ContinuousExtremaMetricAccumulator::default(),
            tps: ContinuousExtremaMetricAccumulator::default(),
            tps_start: now,
        }
    }
}
