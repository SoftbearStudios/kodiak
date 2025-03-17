// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{SocketUpdate, State};
use crate::bitcode::{DecodeOwned, Encode};
use crate::js_hooks::console_error;
use kodiak_common::{decode_buffer, encode_buffer, Compression, CompressionImpl, Decompressor};
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};
use yew::Callback;

struct ProtoWebSocketInner<I, O> {
    socket: WebSocket,
    state: State,
    updated: bool,
    inbound: Callback<SocketUpdate<I>>,
    /// Only used in State::Opening.
    outbound_buffer: Vec<O>,
    decompressor: <CompressionImpl as Compression>::Decompressor,
}

impl<I, O> ProtoWebSocketInner<I, O> {
    fn finalize(&mut self, fin: State) {
        self.state.finalize(fin, &self.inbound);
    }
}

/// Websocket that obeys a protocol consisting of an inbound and outbound message.
pub struct ProtoWebSocket<I, O> {
    inner: Rc<RefCell<ProtoWebSocketInner<I, O>>>,
}

impl<I, O> ProtoWebSocket<I, O>
where
    I: 'static + DecodeOwned,
    O: 'static + Encode,
{
    /// Opens a new websocket.
    pub(crate) fn new(host: &str, inbound: Callback<SocketUpdate<I>>) -> Self {
        let ret = Self {
            inner: Rc::new(RefCell::new(ProtoWebSocketInner {
                socket: WebSocket::new(host).unwrap(),
                inbound,
                outbound_buffer: Vec::new(),
                updated: false,
                state: State::Opening,
                decompressor: Default::default(),
            })),
        };

        let local_inner_rc = Rc::clone(&ret.inner);
        let local_inner = local_inner_rc.deref().borrow_mut();

        let inner_copy = Rc::clone(&ret.inner);

        let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
            let mut inner = inner_copy.deref().borrow_mut();

            if inner.state.is_error() || inner.state.is_closed() || inner.state.is_dropped() {
                // Do not emit!
                return;
            }

            // Handle difference Text/Binary,...
            let result = if let Ok(array_buffer) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                //console_log!("message event, received arraybuffer: {:?}", abuf);
                let compressed = js_sys::Uint8Array::new(&array_buffer).to_vec();
                inner
                    .decompressor
                    .decompress(&compressed)
                    .map_err(|_| "decompress error".to_owned())
                    .and_then(|decompressed| {
                        decode_buffer(&decompressed).map_err(|e| e.to_string())
                    })
            } else {
                console_error!("message event, received Unknown: {:?}", e.data());
                return;
            };

            match result {
                Ok(update) => {
                    inner.updated = true;
                    inner.inbound.emit(SocketUpdate::Inbound(update));
                }
                Err(e) => {
                    console_error!("error decoding websocket data: {}", e);
                    // Mark as closed without actually closing. This may keep a player's session
                    // alive for longer, so they can save their progress by refreshing. The
                    // refresh menu should encourage this.
                    inner.finalize(State::Closed);
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        // set message event handler on WebSocket
        local_inner
            .socket
            .set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        // forget the callback to keep it alive
        onmessage_callback.forget();

        let inner_copy = Rc::clone(&ret.inner);
        let onerror_callback = Closure::wrap(Box::new(move |_e: ErrorEvent| {
            // This will be followed by a close even, which is reported to the caller by
            // handle_close
            inner_copy.borrow_mut().finalize(State::Error);
        }) as Box<dyn FnMut(ErrorEvent)>);
        local_inner
            .socket
            .set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let inner_copy = Rc::clone(&ret.inner);
        let onopen_callback = Closure::once(move || {
            let mut inner = inner_copy.deref().borrow_mut();
            if !inner.state.is_opening() {
                return;
            }
            inner.state = State::Open;
            for outbound in std::mem::take(&mut inner.outbound_buffer) {
                Self::do_send(&inner.socket, outbound);
            }
        });
        local_inner
            .socket
            .set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        let inner_copy = Rc::clone(&ret.inner);
        let onclose_callback = Closure::once(move |e: CloseEvent| {
            let fin = if e.code() == 1000 {
                State::Closed
            } else {
                State::Error
            };
            inner_copy.borrow_mut().finalize(fin);
        });
        local_inner
            .socket
            .set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        local_inner
            .socket
            .set_binary_type(web_sys::BinaryType::Arraybuffer);

        ret
    }

    /// Gets current (cached) websocket state.
    pub(crate) fn state(&self) -> State {
        self.inner.borrow().state
    }

    pub(crate) fn take_updated(&self) -> bool {
        std::mem::take(&mut self.inner.borrow_mut().updated)
    }

    /// How many items + bytes are queued to send.
    pub(crate) fn outbound_backlog(&self) -> usize {
        let inner = self.inner.borrow();
        inner
            .outbound_buffer
            .len()
            .saturating_add(inner.socket.buffered_amount() as usize)
    }

    /// Send a message or buffer reliable messages if the websocket is still opening.
    pub(crate) fn send(&mut self, msg: O, reliable: bool) {
        let mut inner = self.inner.deref().borrow_mut();
        match inner.state {
            State::Opening => {
                if reliable {
                    inner.outbound_buffer.push(msg);
                } else {
                    // Hack? This helps mazean recover from new connection.
                }
            }
            State::Open => {
                Self::do_send(&inner.socket, msg);
            }
            s => console_error!("cannot send on {s:?} websocket"),
        }
    }

    /// Sends a message or drop it on error.
    fn do_send(socket: &WebSocket, msg: O) {
        let buf = encode_buffer(&msg);
        if socket.send_with_u8_array(&buf).is_err() {
            console_error!("error sending binary on ws");
        }
    }
}

impl<I, O> ProtoWebSocket<I, O> {
    /// Close the connection
    pub(crate) fn close(&mut self) {
        let inner = self.inner.deref().borrow();
        // Calling close may synchronously invoke onerror, which borrows inner. Must drop
        // our borrow first.
        let clone = inner.socket.clone();
        drop(inner);
        let _ = clone.close();
    }

    /// Close the connection as if it had an error.
    pub fn error(&mut self) {
        {
            self.inner.borrow_mut().finalize(State::Error);
        }
        self.close();
    }
}

impl<I, O> Drop for ProtoWebSocket<I, O> {
    fn drop(&mut self) {
        self.inner.borrow_mut().finalize(State::Dropped);
    }
}
