// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::bitcode::*;
use crate::broker::Apply;
use crate::js_hooks;
use crate::net::{ProtoSocket, State};
use std::marker::PhantomData;
use yew::Callback;

use super::socket::SocketUpdate;

/// Reconnectable Socket (generic over inbound, outbound, state, and transport).
/// Old state is preserved after closing, but cleared when a new connection is reopened.
pub struct ReconnSocket<I, O, S> {
    inner: ProtoSocket<I, O>,
    /// For when we need to retry.
    socket_inbound: Callback<SocketUpdate<I>>,
    host: String,
    tries: u8,
    next_try: f32,
    /// Last time received an inbound message or outbound backlog shrunk.
    last_progress: f32,
    /// Last outbound backlog size.
    last_outbound_backlog: usize,
    _spooky: PhantomData<S>,
}

impl<I, O, S> ReconnSocket<I, O, S>
where
    I: 'static + DecodeOwned,
    O: 'static + Encode,
    S: Apply<I>,
{
    const MAX_TRIES: u8 = 5;
    const SECONDS_PER_TRY: f32 = 1.0;

    pub(crate) fn new(
        host: String,
        try_web_transport: bool,
        socket_inbound: Callback<SocketUpdate<I>>,
    ) -> Self {
        Self {
            inner: ProtoSocket::new(&host, try_web_transport, socket_inbound.clone()),
            socket_inbound,
            host,
            tries: 0,
            next_try: 0.0,
            last_progress: 0.0,
            last_outbound_backlog: 0,
            _spooky: PhantomData,
        }
    }

    /// Returns whether the underlying connection is closed (for any reason).
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Returns whether the underlying connection is open.
    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }

    pub fn is_reconnecting(&self) -> bool {
        matches!(self.inner.state(), State::Opening | State::Error)
            && (1..=Self::MAX_TRIES).contains(&self.tries)
    }

    /// Returns whether the underlying connection is closed and reconnection attempts have been
    /// exhausted.
    pub fn is_terminated(&self) -> bool {
        self.inner.state() == State::Closed
            || (self.inner.is_error() && self.tries >= Self::MAX_TRIES)
    }

    /// Takes the current time, and returns a collection of updates to apply to the current
    /// state. Will automatically reconnect and clear state if/when the underlying connection is new.
    ///
    /// TODO: Until further notice, it is the caller's responsibility to apply the state changes.
    pub fn update(&mut self, state: &mut S, time_seconds: f32) {
        if self.inner.take_updated() {
            self.last_progress = time_seconds;
        }
        self.reconnect_if_necessary(state, time_seconds);
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    /// Reset the host (for future connections) to a different value.
    pub fn reset_host(&mut self, host: String) {
        self.host = host;
    }

    /// Sends a message, or queues it for sending when the underlying connection is open.
    pub fn send(&mut self, msg: O, reliable: bool) {
        self.inner.send(msg, reliable);
    }

    /// Attempts to reestablish a connection if necessary. This does not and should not preserve
    /// pending messages.
    fn reconnect_if_necessary(&mut self, state: &mut S, time_seconds: f32) {
        if self.inner.state() == State::Open {
            if self.tries > 0 {
                // Reconnected, forget state/tries.
                js_hooks::console_log!(
                    "reconnected socket after {} attempts (unreliable = {}).",
                    self.tries,
                    self.inner.supports_unreliable()
                );
                // Used to reset here but now unnecessary since the inner socket will `Close`.
                self.tries = 0;
                self.last_progress = time_seconds;
                self.next_try = time_seconds + Self::SECONDS_PER_TRY * 0.5;
            } else {
                let backlog = self.inner.outbound_backlog();
                if backlog < self.last_outbound_backlog {
                    self.last_progress = time_seconds;
                }
                self.last_outbound_backlog = backlog;
                if self.last_progress > 0.0 {
                    let since_progress = time_seconds - self.last_progress;
                    if since_progress > 25.0 {
                        js_hooks::console_error!("WS made no recent progress; closing");
                        self.inner.error();
                    } else if since_progress > 1.0 {
                        js_hooks::console_log!(
                            "WS made no progress in the last {since_progress:.0}s"
                        );
                    }
                }
            }
        } else if time_seconds < self.next_try {
            // Wait...
        } else if self.inner.is_error() && self.tries < Self::MAX_TRIES {
            // Try again.
            self.inner = ProtoSocket::new(&self.host, false, self.socket_inbound.clone());
            self.next_try = time_seconds + Self::SECONDS_PER_TRY * 1.8f32.powi(self.tries as i32);
            self.tries += 1;
        } else if self.is_terminated() {
            // Stop trying, stop giving the impression of working.
            if self.tries == Self::MAX_TRIES {
                js_hooks::console_log!("gave up on reconnects");
                // Don't print again.
                self.tries += 1;
            }
            state.reset();
        }
    }

    /// Drop, but leave open the possibility of auto-reconnecting (useful for testing Self).
    pub fn simulate_drop(&mut self) {
        self.inner.close();
    }

    pub fn supports_unreliable(&self) -> bool {
        self.inner.supports_unreliable()
    }
}

impl<I, O, S> Drop for ReconnSocket<I, O, S> {
    fn drop(&mut self) {
        self.inner.close();
    }
}
