// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{SocketUpdate, State};
use crate::bitcode::{DecodeOwned, Encode};
use crate::js_hooks::{self, console_error, window};
use crate::{decode_buffer, encode_buffer, Compression, CompressionImpl, Decompressor};
use js_sys::{Array, Reflect, Uint8Array};
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    ReadableStreamDefaultReader, WebTransport, WebTransportBidirectionalStream,
    WebTransportCloseInfo, WebTransportCongestionControl, WebTransportHash, WebTransportOptions,
    WritableStreamDefaultWriter,
};
use yew::Callback;

struct ProtoWebTransportInner<I, O> {
    connection: WebTransport,
    reliable_reader: Option<ReadableStreamDefaultReader>,
    reliable_reader_buffer: Vec<u8>,
    reliable_writer: Option<WritableStreamDefaultWriter>,
    unreliable_reader: ReadableStreamDefaultReader,
    unreliable_writer: WritableStreamDefaultWriter,
    updated: bool,
    state: State,
    inbound: Callback<SocketUpdate<I>>,
    /// Only used in State::Opening.
    outbound_buffer: Vec<O>,
    decompressor: <CompressionImpl as Compression>::Decompressor,
}

impl<I, O> ProtoWebTransportInner<I, O> {
    fn finalize(&mut self, fin: State) {
        self.state.finalize(fin, &self.inbound)
    }
}

/// WebTransport session that obeys a protocol consisting of an inbound and outbound message.
pub struct ProtoWebTransport<I, O> {
    inner: Rc<RefCell<ProtoWebTransportInner<I, O>>>,
}

impl<I, O> ProtoWebTransport<I, O>
where
    I: 'static + DecodeOwned,
    O: 'static + Encode,
{
    /// Opens a new websocket.
    pub(crate) fn new(host: &str, inbound: Callback<SocketUpdate<I>>) -> Result<Self, ()> {
        if window().get("WebTransport").is_none() {
            js_hooks::console_log!("webtransport unsupported");
            return Err(());
        }
        if host.starts_with("ws://") {
            js_hooks::console_log!("webtransport must be encrypted");
            return Err(());
        }
        let (url, query) = host.split_once('?').unwrap_or((host, ""));
        let url = url.trim_start_matches("wss://").trim_end_matches("/ws");
        let self_signed = url.starts_with("localhost") || url.starts_with("127.0.0.1");
        let mut url = "https://".to_owned() + url + "?" + query;
        if !url.contains(':') {
            // always need port.
            url.push_str(":443");
        }

        let options = WebTransportOptions::new();
        options.set_allow_pooling(false);
        options.set_congestion_control(WebTransportCongestionControl::LowLatency);
        options.set_require_unreliable(true);
        if self_signed {
            let hash = WebTransportHash::new();
            hash.set_algorithm("sha-256");
            hash.set_value(unsafe { &Uint8Array::view(include_bytes!("./certificate_hash.bin")) });
            options.set_server_certificate_hashes(&Array::of1(&hash));
        }

        let connection = WebTransport::new_with_options(&url, &options).map_err(|e| {
            js_hooks::console_log!("webtransport: {e:?}");
            ()
        })?;
        let datagram = connection.datagrams();
        datagram.set_incoming_max_age(2000.0);
        datagram.set_outgoing_max_age(2000.0);
        let ret = Self {
            inner: Rc::new(RefCell::new(ProtoWebTransportInner {
                reliable_reader: None,
                reliable_reader_buffer: Default::default(),
                reliable_writer: None,
                unreliable_reader: datagram
                    .readable()
                    .get_reader()
                    .dyn_into::<ReadableStreamDefaultReader>()
                    .unwrap(),
                unreliable_writer: datagram.writable().get_writer().unwrap(),
                connection: connection.clone(),
                inbound,
                outbound_buffer: Vec::new(),
                updated: false,
                state: State::Opening,
                decompressor: Default::default(),
            })),
        };

        let connection_clone = connection.clone();
        let clone = Rc::clone(&ret.inner);
        let _ = future_to_promise(async move {
            // just use the error from the next result.
            if JsFuture::from(connection_clone.ready()).await.is_err() {
                clone.borrow_mut().finalize(State::Error);
                return Ok(JsValue::NULL);
            }
            let result = JsFuture::from(connection_clone.create_bidirectional_stream()).await;
            let mut inner = clone.borrow_mut();
            let reliable_value = match result {
                Ok(s) => s,
                Err(_) => {
                    console_error!("could not create bidi stream");
                    inner.finalize(State::Error);
                    return Ok(JsValue::NULL);
                }
            };
            let reliable = reliable_value
                .dyn_into::<WebTransportBidirectionalStream>()
                .unwrap();

            inner.reliable_reader = Some(
                reliable
                    .readable()
                    .get_reader()
                    .dyn_into::<ReadableStreamDefaultReader>()
                    .unwrap(),
            );
            inner.reliable_writer = Some(reliable.writable().get_writer().unwrap());
            if !inner.state.is_opening() {
                return Ok(JsValue::NULL);
            }
            inner.state = State::Open;

            for outbound in std::mem::take(&mut inner.outbound_buffer) {
                let mut send_buf = Self::send_buf(&outbound);
                Self::prepend_len(&mut send_buf);
                Self::do_write(inner.reliable_writer.as_ref().unwrap(), &send_buf);
            }

            let reliable_reader = inner.reliable_reader.clone().unwrap();
            drop(inner);

            'outer: loop {
                let Ok(read) = JsFuture::from(reliable_reader.read()).await else {
                    clone.borrow_mut().finalize(State::Error);
                    break;
                };
                let done = Reflect::get(&read, &JsValue::from_str("done"))
                    .unwrap()
                    .as_bool()
                    .unwrap();

                let mut inner = clone.borrow_mut();

                if done {
                    inner.finalize(State::Closed);
                    break;
                }

                if inner.state.is_closed() || inner.state.is_error() || inner.state.is_dropped() {
                    // Too late, do not emit!
                    break;
                }

                let value = Reflect::get(&read, &JsValue::from_str("value"))
                    .unwrap()
                    .dyn_into::<Uint8Array>()
                    .unwrap();

                inner.reliable_reader_buffer.append(&mut value.to_vec());

                loop {
                    let Some(&len_bytes) = inner.reliable_reader_buffer.array_chunks().next()
                    else {
                        // Can't read len yet.
                        continue 'outer;
                    };

                    let len = u32::from_be_bytes(len_bytes) as usize;

                    if inner.reliable_reader_buffer.len() < 4 + len {
                        // Can't ready message yet.
                        continue 'outer;
                    }

                    let rest = inner.reliable_reader_buffer.split_off(4 + len);
                    let len_message = std::mem::replace(&mut inner.reliable_reader_buffer, rest);

                    let result = inner
                        .decompressor
                        .decompress(&len_message[4..])
                        .map_err(|_| "decompress error".to_owned())
                        .and_then(|decompressed| {
                            decode_buffer(&decompressed).map_err(|e| e.to_string())
                        });

                    match result {
                        Ok(update) => {
                            inner.updated = true;
                            inner.inbound.emit(SocketUpdate::Inbound(update))
                        }
                        Err(e) => {
                            console_error!("error decoding webtransport data: {}", e);
                            // Mark as closed without actually closing. This may keep a player's session
                            // alive for longer, so they can save their progress by refreshing. The
                            // refresh menu should encourage this.
                            inner.finalize(State::Closed);
                            break 'outer;
                        }
                    }
                }
            }

            Ok(JsValue::NULL)
        });

        let connection_clone = connection.clone();
        let clone = Rc::clone(&ret.inner);
        let _ = future_to_promise(async move {
            let result = JsFuture::from(connection_clone.ready()).await;
            let mut inner = clone.borrow_mut();
            if result.is_err() {
                inner.finalize(State::Error);
                return Ok(JsValue::NULL);
            }
            let unreliable_reader = inner.unreliable_reader.clone();
            drop(inner);

            loop {
                let Ok(read) = JsFuture::from(unreliable_reader.read()).await else {
                    clone.borrow_mut().finalize(State::Error);
                    break;
                };
                let done = Reflect::get(&read, &JsValue::from_str("done"))
                    .unwrap()
                    .as_bool()
                    .unwrap();

                let mut inner = clone.borrow_mut();
                if done {
                    inner.finalize(State::Closed);
                    break;
                }

                if inner.state.is_closed() || inner.state.is_error() || inner.state.is_dropped() {
                    // Too late, do not emit!
                    break;
                }

                let value = Reflect::get(&read, &JsValue::from_str("value"))
                    .unwrap()
                    .dyn_into::<Uint8Array>()
                    .unwrap();

                let compressed = value.to_vec();
                let result = CompressionImpl::decompress(&compressed)
                    .map_err(|_| "decompress error".to_owned())
                    .and_then(|decompressed| {
                        decode_buffer(&decompressed).map_err(|e| e.to_string())
                    });

                match result {
                    Ok(update) => {
                        inner.updated = true;
                        inner.inbound.emit(SocketUpdate::Inbound(update));
                    }
                    Err(e) => {
                        console_error!("error decoding webtransport data: {}", e);
                        // Mark as closed without actually closing. This may keep a player's session
                        // alive for longer, so they can save their progress by refreshing. The
                        // refresh menu should encourage this.
                        inner.finalize(State::Closed);
                        break;
                    }
                }
            }
            // Error: inner_copy.deref().borrow_mut().state = State::Error;

            Ok(JsValue::NULL)
        });

        let connection_clone = connection.clone();
        let clone = Rc::clone(&ret.inner);
        let _ = future_to_promise(async move {
            let result = JsFuture::from(connection_clone.closed()).await;
            let mut inner = clone.borrow_mut();
            let Ok(close) = result else {
                inner.finalize(State::Error);
                return Ok(JsValue::NULL);
            };
            let code = Reflect::get(&close, &JsValue::from_str("closeCode"))?
                .as_f64()
                .unwrap() as u64;
            js_hooks::console_log!("WT debug: close code = {code} state = {:?}", inner.state);
            let fin = if code == 0 {
                State::Closed
            } else {
                State::Error
            };
            inner.finalize(fin);
            Ok(JsValue::NULL)
        });

        Ok(ret)
    }

    pub(crate) fn take_updated(&self) -> bool {
        std::mem::take(&mut self.inner.borrow_mut().updated)
    }

    /// Gets current (cached) websocket state.
    pub(crate) fn state(&self) -> State {
        self.inner.borrow().state
    }

    /// How many items + bytes are queued to send.
    pub(crate) fn outbound_backlog(&self) -> usize {
        /*
        let inner = self.inner.borrow();
        inner
            .outbound_buffer
            .len()
            .saturating_add(inner.socket.buffered_amount() as usize)
        */
        0 // TODO
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
                let mut send_buf = Self::send_buf(&msg);
                let writer = if reliable
                    || send_buf.len() > inner.connection.datagrams().max_datagram_size() as usize
                {
                    Self::prepend_len(&mut send_buf);
                    inner.reliable_writer.as_ref().unwrap()
                } else {
                    &inner.unreliable_writer
                };
                Self::do_write(writer, &send_buf);
            }
            s => console_error!("cannot send on {s:?} webtransport"),
        }
    }

    fn send_buf<T: Encode>(msg: &T) -> Vec<u8> {
        encode_buffer(msg)
    }

    fn prepend_len(buf: &mut Vec<u8>) {
        buf.splice(..0, (buf.len() as u32).to_be_bytes());
    }

    /// Caller responsible for prepending length for reliable writes.
    fn do_write(writer: &WritableStreamDefaultWriter, buf: &[u8]) {
        let chunk = Uint8Array::new_with_length(buf.len() as u32);
        chunk.copy_from(buf);
        // TODO: Error handling.
        // `write*` waits for previous successful writes.
        let _ = writer.write_with_chunk(&chunk);
    }
}

impl<I, O> ProtoWebTransport<I, O> {
    /// Close the connection
    pub(crate) fn close(&mut self) {
        let inner = self.inner.deref().borrow();
        let clone = inner.connection.clone();
        drop(inner);
        let info = WebTransportCloseInfo::new();
        info.set_close_code(0);
        let _ = clone.close_with_close_info(&info);
    }

    /// Close the connection as if it had an error.
    pub(crate) fn error(&mut self) {
        let mut inner = self.inner.borrow_mut();
        inner.finalize(State::Error);
        drop(inner);
        self.close();
    }
}

impl<I, O> Drop for ProtoWebTransport<I, O> {
    fn drop(&mut self) {
        let mut inner = self.inner.borrow_mut();
        inner.finalize(State::Dropped);
    }
}
