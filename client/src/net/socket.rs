// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::web_socket::ProtoWebSocket;
use super::web_transport::ProtoWebTransport;
use crate::bitcode::*;
use yew::Callback;

/// The state of a socket.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum State {
    Opening,
    Open,
    Error,
    Closed,
    Dropped,
}

#[derive(Debug)]
pub enum SocketUpdate<I> {
    /// Inbound message received on the socket.
    Inbound(I),
    /// The socket is done producing inbound's.
    Closed,
}

#[allow(unused)]
impl State {
    pub fn is_opening(self) -> bool {
        matches!(self, Self::Opening)
    }

    pub fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }

    pub fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }

    pub fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }

    pub fn is_dropped(self) -> bool {
        matches!(self, Self::Dropped)
    }

    pub fn finalize<I>(&mut self, fin: Self, callback: &Callback<SocketUpdate<I>>) {
        debug_assert!(matches!(fin, Self::Closed | Self::Error | Self::Dropped));
        if matches!(self, Self::Opening | Self::Open) {
            *self = fin;
            callback.emit(SocketUpdate::Closed);
        }
    }
}

pub enum ProtoSocket<I, O> {
    WebSocket(ProtoWebSocket<I, O>),
    WebTransport(ProtoWebTransport<I, O>),
}

impl<I, O> ProtoSocket<I, O>
where
    I: 'static + DecodeOwned,
    O: 'static + Encode,
{
    pub(crate) fn new(
        host: &str,
        web_transport: bool,
        socket_inbound: Callback<SocketUpdate<I>>,
    ) -> Self {
        if web_transport
            && let Ok(web_transport) = ProtoWebTransport::new(host, socket_inbound.clone())
        {
            Self::WebTransport(web_transport)
        } else {
            Self::WebSocket(ProtoWebSocket::new(host, socket_inbound))
        }
    }

    pub(crate) fn supports_unreliable(&self) -> bool {
        matches!(self, Self::WebTransport(_))
    }

    pub(crate) fn take_updated(&self) -> bool {
        match self {
            Self::WebSocket(web_socket) => web_socket.take_updated(),
            Self::WebTransport(web_transport) => web_transport.take_updated(),
        }
    }

    /// Gets current (cached) socket state.
    pub(crate) fn state(&self) -> State {
        match self {
            Self::WebSocket(web_socket) => web_socket.state(),
            Self::WebTransport(web_transport) => web_transport.state(),
        }
    }

    /// How many items + bytes are queued to send.
    pub(crate) fn outbound_backlog(&self) -> usize {
        match self {
            Self::WebSocket(web_socket) => web_socket.outbound_backlog(),
            Self::WebTransport(web_transport) => web_transport.outbound_backlog(),
        }
    }

    /// Returns whether closed for any reason (error or not).
    pub(crate) fn is_closed(&self) -> bool {
        matches!(self.state(), State::Closed | State::Error)
    }

    /// Returns whether closed in error.
    pub(crate) fn is_error(&self) -> bool {
        matches!(self.state(), State::Error)
    }

    /// Returns whether socket is open.
    pub(crate) fn is_open(&self) -> bool {
        matches!(self.state(), State::Open)
    }

    /// Send a message or buffer reliable messages if the websocket is still opening.
    pub(crate) fn send(&mut self, msg: O, reliable: bool) {
        match self {
            Self::WebSocket(web_socket) => web_socket.send(msg, reliable),
            Self::WebTransport(web_transport) => web_transport.send(msg, reliable),
        }
    }
}

impl<I, O> ProtoSocket<I, O> {
    pub(crate) fn close(&mut self) {
        match self {
            Self::WebSocket(web_socket) => web_socket.close(),
            Self::WebTransport(web_transport) => web_transport.close(),
        }
    }

    /// Close the connection as if it had an error.
    pub(crate) fn error(&mut self) {
        match self {
            Self::WebSocket(web_socket) => web_socket.error(),
            Self::WebTransport(web_transport) => web_transport.error(),
        }
    }
}
