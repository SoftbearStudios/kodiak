// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::disposition::LockstepDispositionInner;
use super::{
    Lockstep, LockstepInputId, LockstepInputQueue, LockstepInputWindow, LockstepRequest,
    LockstepTick, LockstepUpdate, LockstepWorld,
};
use crate::lockstep::disposition::LockstepDisposition;
use crate::{ArenaMap, PlayerId};
use heapless::HistoryBuffer;

pub struct LockstepClient<W: LockstepWorld>
where
    [(); W::MAX_PREDICTION]:,
    [(); W::LAG_COMPENSATION]:,
    [(); W::TPS]:,
{
    pub real: Lockstep<W>,
    /// `None` before initial complete received, otherwise `Some`.
    pub player_id: Option<PlayerId>,
    pub input_queue: LockstepInputQueue<W>,
    pub predicted: Lockstep<W>,
    pub predicted_next: Lockstep<W>,
    pub interpolated: Lockstep<W>,
    pub heard_from_server: bool,
    /// Fractional ticks since last predicted tick.
    pub since_predicted_tick: f32,
    pub smoothed_normalized_ticks_since_real: f32,
    pub server_buffered_inputs: usize,
    pub ping_latencies: HistoryBuffer<u32, { W::TPS }>,
    pub total_latencies: HistoryBuffer<u32, { W::TPS }>,
    pub(crate) info: Vec<W::Info>,
}

impl<W: LockstepWorld + Default> Default for LockstepClient<W>
where
    [(); W::MAX_PREDICTION]:,
    [(); W::LAG_COMPENSATION]:,
    [(); W::TPS]:,
{
    fn default() -> Self {
        Self {
            real: Default::default(),
            player_id: Default::default(),
            input_queue: Default::default(),
            predicted: Default::default(),
            predicted_next: Default::default(),
            interpolated: Default::default(),
            heard_from_server: false,
            since_predicted_tick: 0.0,
            smoothed_normalized_ticks_since_real: 0.0,
            server_buffered_inputs: 0,
            ping_latencies: Default::default(),
            total_latencies: Default::default(),
            info: Default::default(),
        }
    }
}

impl<W: LockstepWorld> LockstepClient<W>
where
    [(); W::MAX_PREDICTION]:,
    [(); W::INPUTS_PER_EFFICIENT_PACKET]:,
    [(); W::LAG_COMPENSATION]:,
    [(); W::TPS]:,
{
    /// Returns ping latency and total latency.
    pub fn receive(&mut self, update: LockstepUpdate<W>) -> (Option<u32>, Option<u32>) {
        if let Some((player_id, initialization)) = update.initialization {
            self.player_id = Some(player_id);
            self.real.clone_from(&initialization);
            self.predicted.clone_from(&initialization);
            self.predicted_next.clone_from(&initialization);
            self.interpolated = initialization;
            // Kludge.
            //self.input_queue = Default::default();

            if let Some(info) = W::on_complete() {
                self.info.push(info);
            }
        }
        self.server_buffered_inputs = update.buffered_inputs;
        let (ping_latency, total_latency) = self.tick(
            update.tick,
            update.last_applied_input_id,
            update.last_received_input_id,
        );
        //#[cfg(feature = "log")]
        //log::info!("ping = {ping_latency:?} total = {total_latency:?}");
        if let Some(ping_latency) = ping_latency {
            self.ping_latencies.write(ping_latency);
        }
        if let Some(total_latency) = total_latency {
            self.total_latencies.write(total_latency);
        }
        (ping_latency, total_latency)
    }

    /// Advances the real world by one `tick` from the server. Also corrects any miss predictions
    /// we made with [`Self::tick_predicted`].
    ///
    /// Uses a `checksum` to catch desyncs due to non-deterministic code.
    ///
    /// Returns ping latency (ticks) and total latency.
    pub(crate) fn tick(
        &mut self,
        tick: LockstepTick<W>,
        last_applied_id: LockstepInputId,
        last_received_id: LockstepInputId,
    ) -> (Option<u32>, Option<u32>) {
        assert!(
            last_received_id < self.input_queue.end && last_applied_id < self.input_queue.end,
            "server received/applied unsent command {last_received_id}/{last_applied_id} >= {}",
            self.input_queue.end
        );

        if let Some(checksum) = tick.checksum {
            let real = self.real.checksum();
            if real != checksum {
                #[cfg(feature = "desync")]
                {
                    log::warn!("client = {:?}", self.real);
                    log::warn!("server = {:?}", tick.complete);
                }
                #[cfg(feature = "log")]
                log::error!("desync {real} {checksum}");
                panic!("desync {real} {checksum}");
            }
            assert_eq!(real, checksum, "desync");
        }
        self.input_queue.acknowledged(last_applied_id);
        self.heard_from_server = true;
        self.smoothed_normalized_ticks_since_real -= 1.0;

        // Lockstep.
        self.real.tick(
            tick,
            &LockstepDisposition {
                inner: LockstepDispositionInner::GroundTruth,
            },
            &mut |info| {
                if let Some(player_id) = self.player_id
                    && !W::is_predicted(&info, player_id)
                {
                    self.info.push(info);
                }
            },
        );

        // Prediction.
        let old_predicted = std::mem::replace(&mut self.predicted, self.real.clone());
        for c in self.input_queue.iter() {
            Self::predict(
                &mut self.predicted,
                self.player_id,
                false,
                Some(c),
                &mut |_| {},
            );
        }
        if W::INTERPOLATE_PREDICTION {
            let old_predicted =
                if self.predicted_next.context.tick_id == self.predicted.context.tick_id {
                    &self.predicted_next
                } else {
                    &old_predicted
                };
            self.predicted = old_predicted.lerp(
                &self.predicted,
                (W::TICK_PERIOD_SECS * 2.0).min(1.0),
                &LockstepDisposition {
                    inner:
                        LockstepDispositionInner::LerpingOldCurrentPredictionToNewCurrentPrediction,
                },
            );
        }

        // Prediction next.
        self.predicted_next = self.predicted.clone();
        let _old_predicted_next =
            std::mem::replace(&mut self.predicted_next, self.predicted.clone());
        Self::predict(
            &mut self.predicted_next,
            self.player_id,
            true,
            None,
            &mut |_| {},
        );

        // Interpolation.
        // TODO: need to recalculate time since prediction.
        self.update_interpolated();

        // 0 is used when server is missing a command, may be too high if we sent to the old server
        // while on the new server during server switching.
        (
            (last_received_id != 0).then(|| self.input_queue.latency(last_received_id)),
            (last_applied_id != 0).then(|| self.input_queue.latency(last_applied_id)),
        )
    }

    fn update_interpolated(&mut self) {
        self.interpolated = self.predicted.lerp(
            &self.predicted_next,
            self.since_predicted_tick,
            &LockstepDisposition {
                inner: LockstepDispositionInner::LerpingCurrentPredictionToNextPrediction {
                    perspective: self.player_id,
                    smoothed_normalized_ticks_since_real: self.smoothed_normalized_ticks_since_real,
                },
            },
        );
    }

    pub fn update<'a>(
        &'a mut self,
        elapsed_seconds: f32,
        supports_unreliable: bool,
        mut input: impl FnMut(bool) -> W::Input,
        mut send_with_reliable: impl FnMut(LockstepRequest<W>, bool),
    ) -> impl Iterator<Item = W::Info> + 'a
    where
        W::Player: std::fmt::Debug,
        W::Input: std::fmt::Debug,
        W: std::fmt::Debug,
    {
        if !self.loaded() {
            return self.info.drain(..);
        }
        let server_buffer_usage = self.server_buffer_usage();
        let client_buffer_usage = self.client_buffer_usage();
        let buffer_usage = server_buffer_usage * 0.5 + client_buffer_usage * 0.5;
        // https://www.desmos.com/calculator/dgsdlh0yng
        let tick_scale = 1.0
            + 0.25
                * (2.0 * (buffer_usage - W::target_buffer(supports_unreliable)))
                    .clamp(-1.0, 1.0)
                    .tan();
        let adjusted_tick_period_secs = W::TICK_PERIOD_SECS * tick_scale;
        //#[cfg(feature = "log")]
        //log::info!("CBUFFER {client_buffer_usage:.2} SBUFFER {server_buffer_usage:.2} TICK {tick_scale:.2}");
        self.since_predicted_tick += elapsed_seconds / adjusted_tick_period_secs;
        let mut whole = (self.since_predicted_tick as usize).min(W::BUFFERED_TICKS);
        // Limited to `BUFFERED_TICKS` so if you tab out we don't spam the server with messages.
        // TODO reduce latency by aiming for messages to arrive right when server ticks.

        for i in 0..whole {
            let input = input(true);

            if let Ok(inputs) = self.tick_predicted(input, supports_unreliable) {
                send_with_reliable(LockstepRequest { inputs }, false);
            } else {
                //#[cfg(feature = "log")]
                //log::warn!("unable to predict");
                whole = i;
                break;
            }
        }

        // even if predicted didn't change, input may have.
        self.since_predicted_tick =
            (self.since_predicted_tick - whole as f32).clamp(0.0, 2.0 / adjusted_tick_period_secs);

        self.predicted_next.clone_from(&self.predicted);
        Self::predict(
            &mut self.predicted_next,
            self.player_id,
            true,
            Some(input(false)),
            &mut |_| {},
        );

        //println!("id = {} whole = {whole} spt = {:.2}", self.predicted.context.tick_id, self.since_predicted_tick);

        //#[cfg(feature = "log")]
        //log::info!("whole={whole} fract={:.2}", self.since_predicted_tick);

        self.smoothed_normalized_ticks_since_real += elapsed_seconds * (1.0 / W::TICK_PERIOD_SECS);
        self.smoothed_normalized_ticks_since_real += (-self.smoothed_normalized_ticks_since_real)
            .clamp(
                -elapsed_seconds * (0.1 / W::TICK_PERIOD_SECS),
                elapsed_seconds * (0.1 / W::TICK_PERIOD_SECS),
            );
        self.smoothed_normalized_ticks_since_real =
            self.smoothed_normalized_ticks_since_real.clamp(-1.0, 1.0);

        self.update_interpolated();

        /*
        if whole > 0 {
            log::info!("pred = {:?} pred_next = {:?} lerp = {:?}", self.predicted, self.predicted_next, self.interpolated);
        }
        */

        self.info.drain(..)
    }

    pub fn lag_compensation_latency(&self) -> u8 {
        self.total_latencies
            .recent()
            .copied()
            .unwrap_or(0)
            .min(W::MAX_LATENCY as u32) as u8
    }

    /// Average ping latency over the last second in ticks.
    pub fn average_ping_latency(&self) -> u32 {
        self.ping_latencies
            .iter()
            .sum::<u32>()
            .checked_div(self.ping_latencies.len() as u32)
            .unwrap_or(0)
    }

    /// Average total latency over the last second in ticks.
    pub fn average_total_latency(&self) -> u32 {
        self.total_latencies
            .iter()
            .sum::<u32>()
            .checked_div(self.total_latencies.len() as u32)
            .unwrap_or(0)
    }

    /// Average ping latency over the last second in ticks.
    pub(crate) fn average_ping_latency_secs(&self) -> f32 {
        if self.ping_latencies.is_empty() {
            return 0.0;
        }
        self.ping_latencies.iter().sum::<u32>() as f32 * W::TICK_PERIOD_SECS
            / self.ping_latencies.len() as f32
    }

    pub fn average_ping_latency_ms(&self) -> u32 {
        (self.average_ping_latency_secs() * 1000.0) as u32
    }

    /// 0..=1
    pub fn client_buffer_usage(&self) -> f32 {
        self.input_queue.len() as f32 * (1.0 / W::MAX_PREDICTION as f32)
    }

    /// 0..=1
    pub fn server_buffer_usage(&self) -> f32 {
        self.server_buffered_inputs as f32 * (1.0 / W::BUFFERED_TICKS as f32)
    }

    /// Has the server sent a complete.
    pub fn loaded(&self) -> bool {
        self.player_id.is_some()
    }

    /// Advances the predicted world by one tick given our `controls` and returns a [`CommandWindow`] to
    /// send to the server to ensure our prediction becomes accurate.
    pub(crate) fn tick_predicted(
        &mut self,
        input: W::Input,
        unreliable: bool,
    ) -> Result<LockstepInputWindow<W>, ()> {
        let heard_from_server = std::mem::take(&mut self.heard_from_server);

        // Avoid predicting too much to prevent client from freezing due to too many physics calculations.
        if self.input_queue.is_full() {
            if heard_from_server {
                // We've heard from server but none of our messages have been acknowledged, so we
                // make space in the buffer by deleting an old command. If we dropped the new
                // command instead, the buffer may never empty causing a deadlock.
                // The oldest command is most likely to have been dropped, so we delete it.
                self.input_queue.pop_front();
                #[cfg(feature = "log")]
                log::warn!("full queue, popped front");
            } else {
                // Haven't heard from server since last call to tick_predicted, so we drop the command.
                #[cfg(feature = "log")]
                log::warn!("full queue, haven't heard from server");
                return Err(());
            }
        }

        Self::predict(
            &mut self.predicted,
            self.player_id,
            false,
            Some(input),
            &mut |info| {
                if let Some(player_id) = self.player_id
                    && W::is_predicted(&info, player_id)
                {
                    self.info.push(info);
                }
            },
        );
        Ok(self.input_queue.push_back(input, unreliable))
    }

    /// Advances `world` into the future given one of our `controls`.
    fn predict(
        predicted: &mut Lockstep<W>,
        player_id: Option<PlayerId>,
        interpolation_prediction: bool,
        input: Option<W::Input>,
        on_info: &mut dyn FnMut(W::Info),
    ) {
        let mut inputs = ArenaMap::new();
        if let Some((player_id, input)) = player_id.zip(input) {
            inputs.insert(player_id, input);
        }
        predicted.tick(
            LockstepTick {
                inputs,
                ..Default::default()
            },
            &LockstepDisposition {
                inner: LockstepDispositionInner::Predicting {
                    perspective: player_id,
                    additional_interpolation_prediction: interpolation_prediction,
                },
            },
            on_info,
        );
    }
}
