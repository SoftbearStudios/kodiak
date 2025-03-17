// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{INBOUND_HARD_LIMIT, KEEPALIVE_INTERVAL};
use crate::actor::{ClientAuthErr, ClientAuthRequest};
use crate::router::check_origin;
use crate::service::ArenaService;
use crate::socket::{Socket, SocketMessage, KEEPALIVE_HARD_TIMEOUT};
use crate::state::AppState;
use crate::{Compression, CompressionImpl, Compressor, NonZeroUnixMillis, SocketQuery, UnixTime};
use axum::body::Body;
use axum::extract::{ConnectInfo, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::TypedHeader;
use axum_tws::{CloseCode, Message, WebSocket, WebSocketError, WebSocketUpgrade};
use bytes::Bytes;
use hyper::header::ORIGIN;
use hyper::HeaderMap;
use log::{info, warn};
use std::fmt::Display;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::time::Sleep;

// Used for pongs and binary messages.
// - Rate should account for game messages and engine messages.
// - Burst should account for momentary lapses in connection.

pub async fn ws_request<G: ArenaService>(
    State(state): State<AppState<G>>,
    upgrade: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    user_agent: Option<TypedHeader<axum_extra::headers::UserAgent>>,
    Query(query): Query<SocketQuery>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let Some(origin) = headers
        .get(ORIGIN)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| check_origin::<G>(h))
    else {
        return Err(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::from("invalid origin"))
            .unwrap());
    };

    let user_agent_id = user_agent
        .as_ref()
        .map(|h| h.as_str())
        .or(query.user_agent.as_deref())
        .and_then(|h| crate::net::user_agent_into_id(h));
    let client_auth_request =
        ClientAuthRequest::new::<G>(query, addr.ip(), origin.clone(), user_agent_id);

    let result = state
        .server
        .send(client_auth_request)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
    // Currently, if authentication fails, it was due to rate limit.
    let (arena_id, player_id) = result.map_err(|e| {
        (
            {
                match e {
                    ClientAuthErr::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
                    _ => StatusCode::SERVICE_UNAVAILABLE,
                }
            },
            {
                let e: &'static str = e.into();
                warn!("{e}");
                e
            },
        )
            .into_response()
    })?;

    Ok(upgrade
        .limits(axum_tws::Limits::default().max_payload_len(Some(INBOUND_HARD_LIMIT)))
        .on_upgrade(move |inner| {
            let now = Instant::now();
            let keep_alive = tokio::time::sleep_until((now + KEEPALIVE_INTERVAL).into());
            let web_socket = TokioWebSocket {
                inner,
                keep_alive,
                last_activity: now,
                addr,
                rtt: None,
                compressor: Default::default(),
            };
            async move {
                std::pin::pin!(web_socket)
                    .as_mut()
                    .serve(origin, user_agent_id, arena_id, player_id, state.server)
                    .await;
            }
        }))
}

#[pin_project::pin_project]
struct TokioWebSocket {
    inner: WebSocket,
    #[pin]
    keep_alive: Sleep,
    last_activity: Instant,
    rtt: Option<u16>,
    addr: SocketAddr,
    compressor: <CompressionImpl as Compression>::Compressor,
}

#[derive(Debug)]
enum TokioWebSocketError {
    Internal(WebSocketError),
    Custom(&'static str),
}

impl From<WebSocketError> for TokioWebSocketError {
    fn from(value: WebSocketError) -> Self {
        Self::Internal(value)
    }
}

impl Display for TokioWebSocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal(internal) => Display::fmt(internal, f),
            Self::Custom(custom) => Display::fmt(custom, f),
        }
    }
}

impl std::error::Error for TokioWebSocketError {}

impl Socket for TokioWebSocket {
    type RecvErr = TokioWebSocketError;
    type SendErr = TokioWebSocketError;

    const SUPPORTS_UNRELIABLE: bool = false;

    async fn send(self: Pin<&mut Self>, message: SocketMessage) -> Result<(), Self::SendErr> {
        let this = self.project();
        let ws_message = match message {
            SocketMessage::Reliable(message) | SocketMessage::Unreliable(message) => {
                Message::binary(this.compressor.compress(&message))
            }
            SocketMessage::Close { error } => {
                let code = if error {
                    CloseCode::PROTOCOL_ERROR
                } else {
                    CloseCode::NORMAL_CLOSURE
                };

                Message::close(Some(code), "")
            }
        };
        this.inner
            .send(ws_message)
            .await
            .map_err(TokioWebSocketError::Internal)
    }

    async fn recv(self: Pin<&mut Self>) -> Result<SocketMessage, TokioWebSocketError> {
        let mut this = self.project();
        loop {
            let opt = tokio::select! {
                opt = this.inner.recv() => {
                    opt
                }
                _ = this.keep_alive.as_mut() => {
                    let now = Instant::now();
                    if now - *this.last_activity < KEEPALIVE_HARD_TIMEOUT {
                        let vec: Vec<u8>  = NonZeroUnixMillis::now().to_i64().to_ne_bytes().into();
                        if let Err(e) = this.inner.send(Message::ping(vec)).await {
                            warn!("closing after failed to ping: {e}");
                            return Err(TokioWebSocketError::Custom("failed to ping"));
                        }
                        this.keep_alive.as_mut().reset((now + KEEPALIVE_INTERVAL).into());
                        continue;
                    } else {
                        warn!("closing unresponsive");
                        return Err(TokioWebSocketError::Custom("unresponsive"));
                    }
                }
            };
            let Some(message) = opt else {
                return Err(TokioWebSocketError::Custom("unexpected closure"));
            };
            // Don't reset the keep_alive timer; use pings to measure RTT.
            *this.last_activity = Instant::now();

            let message = message?;

            if message.is_binary() {
                let bytes: Bytes = message.into_payload().into();
                return Ok(SocketMessage::Reliable(bytes));
            } else if message.is_text() {
                return Err(TokioWebSocketError::Custom("unexpected text message"));
            } else if message.is_ping() {
                // TWS will send Pong.
            } else if message.is_pong() {
                let bytes: Bytes = message.into_payload().into();
                if let Ok(bytes) = bytes.as_ref().try_into() {
                    let now = NonZeroUnixMillis::now();
                    let timestamp = NonZeroUnixMillis::from_i64(i64::from_ne_bytes(bytes));
                    let rtt = now.millis_since(timestamp);
                    if rtt <= 10000u64 {
                        *this.rtt = Some(rtt as u16);
                    }
                }
                // continue;
            } else if message.is_close() {
                let code = message.as_close().map(|(c, _)| c);
                info!(
                    "received close from client: {:?}",
                    code.unwrap_or(CloseCode::NO_STATUS_RECEIVED)
                );
                // tokio-websockets will echo close frame if necessary.
                return Ok(SocketMessage::Close {
                    error: code
                        .map(|code| {
                            matches!(
                                code,
                                CloseCode::PROTOCOL_ERROR
                                    | CloseCode::INVALID_FRAME_PAYLOAD_DATA
                                    | CloseCode::UNSUPPORTED_DATA
                                    | CloseCode::INTERNAL_SERVER_ERROR
                                    | CloseCode::MESSAGE_TOO_BIG
                                    | CloseCode::POLICY_VIOLATION
                            )
                        })
                        .unwrap_or(false),
                });
            } else {
                debug_assert!(false);
            }
        }
    }

    fn rtt(&self) -> Option<Duration> {
        self.rtt.map(|ms| Duration::from_millis(ms as u64))
    }

    fn addr(&self) -> SocketAddr {
        self.addr
    }
}
