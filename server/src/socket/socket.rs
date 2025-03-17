// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::{ClientAuthRequest, ServerActor};
use crate::observer::{ObserverMessage, ObserverMessageBody, ObserverUpdate};
use crate::rate_limiter::{RateLimiterProps, RateLimiterState};
use crate::router::AllowedOrigin;
use crate::{
    decode_buffer, encode_buffer, ArenaId, ArenaService, CommonRequest, CommonUpdate, PlayerId,
    UserAgentId,
};
use actix::Addr;
use bytes::Bytes;
use log::{info, warn};
use std::error::Error;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::{Duration, Instant};

/// Max size (in bytes) of an inbound message.
pub const INBOUND_HARD_LIMIT: usize = 16384;
pub const KEEPALIVE_INTERVAL_SECONDS: u64 = 10;
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(KEEPALIVE_INTERVAL_SECONDS);
pub const KEEPALIVE_HARD_TIMEOUT: Duration = Duration::from_secs(KEEPALIVE_INTERVAL_SECONDS * 2);

pub enum SocketMessage {
    /// Guaranteed to be delivered in order.
    ///
    /// Compressed with context.
    Reliable(Bytes),
    /// Not guaranteed to be delivered. May be delivered out of order.
    ///
    /// Compressed without context.
    ///
    /// The socket is allowed to use a `Reliable` message instead.
    Unreliable(Bytes),
    /// Already closed, no need to call `close`.
    ///
    /// # Panics
    ///
    /// Multiple close messages may cause a panic.
    Close { error: bool },
}

pub trait Socket: Sized {
    const SUPPORTS_UNRELIABLE: bool;
    type SendErr: Error;
    type RecvErr: Error;

    async fn send(self: Pin<&mut Self>, message: SocketMessage) -> Result<(), Self::SendErr>;
    async fn recv(self: Pin<&mut Self>) -> Result<SocketMessage, Self::RecvErr>;
    fn addr(&self) -> SocketAddr;
    fn rtt(&self) -> Option<Duration>;
    async fn serve<G: ArenaService>(
        self: Pin<&mut Self>,
        origin: AllowedOrigin,
        user_agent_id: Option<UserAgentId>,
        mut arena_id: ArenaId,
        mut player_id: PlayerId,
        server: Addr<ServerActor<G>>,
    ) {
        let mut this = self;
        let (mut server_sender, mut server_receiver) =
            tokio::sync::mpsc::unbounded_channel::<ObserverUpdate<CommonUpdate<G::GameUpdate>>>();

        server.do_send(
            ObserverMessage {
                arena_id,
                body: ObserverMessageBody::<
                    CommonRequest<G::GameRequest>,
                    CommonUpdate<G::GameUpdate>,
                >::Register {
                    player_id,
                    observer: server_sender.clone(),
                    supports_unreliable: Self::SUPPORTS_UNRELIABLE,
                },
            },
        );

        let tick_period = G::TICK_PERIOD_SECS;
        let inbound_rate_limit_props = RateLimiterProps::new(
            Duration::from_secs_f32(tick_period * 0.75),
            10 + 4 * (1.0 / tick_period).ceil() as u32,
        );

        let mut inbound_rate_limit = RateLimiterState::default();
        let mut rtt_rate_limit = RateLimiterState::default();

        let mut warnings_left = 5u8;

        const RTT_RATE_LIMIT_PROPS: RateLimiterProps =
            RateLimiterProps::const_new(Duration::from_secs(60), 0);

        const CLOSE_OK: Option<bool> = Some(false);
        const CLOSE_ERROR: Option<bool> = Some(true);
        const CLOSE_SILENT: Option<bool> = None;

        let result = loop {
            tokio::select! {
                result = this.as_mut().recv() => {
                    let Ok(message) = result else {
                        break CLOSE_ERROR;
                    };

                    let now = Instant::now();

                    if let Some(rtt) = this.as_ref().rtt() && !rtt_rate_limit.should_limit_rate_with_now(&RTT_RATE_LIMIT_PROPS, now) {
                        server.do_send(ObserverMessage{
                            arena_id,
                            body: ObserverMessageBody::<CommonRequest<G::GameRequest>, CommonUpdate<G::GameUpdate >>::RoundTripTime {
                                player_id,
                                rtt: rtt.as_millis() as u16,
                            }
                        });
                    }

                    let reliable = matches!(message, SocketMessage::Reliable(_));
                    match message {
                        SocketMessage::Reliable(message) | SocketMessage::Unreliable(message) => {
                            //println!("received (burst used {}/{} until {:.2})", inbound_rate_limit.burst_used, inbound_rate_limit_props.burst, (inbound_rate_limit.until - now).as_secs_f32());
                            if inbound_rate_limit.should_limit_rate_with_now(&inbound_rate_limit_props, now) {
                                if let Some(new) = warnings_left.checked_sub(1) {
                                    warnings_left = new;
                                    warn!("{} binary rate-limited", this.as_ref().addr().ip());
                                }
                                tokio::task::yield_now().await;
                                continue;
                            }

                            match decode_buffer(message.as_ref())
                            {
                                Ok(CommonRequest::Redial{query_string}) => {
                                    let (new_server_sender, new_server_receiver) =
                                        tokio::sync::mpsc::unbounded_channel::<ObserverUpdate<CommonUpdate<G::GameUpdate>>>();

                                    server_receiver = new_server_receiver;

                                    server.do_send(ObserverMessage {
                                        arena_id,
                                        body:
                                            ObserverMessageBody::<CommonRequest<G::GameRequest>, CommonUpdate<G::GameUpdate>>::Unregister {
                                                player_id,
                                                observer: std::mem::replace(&mut server_sender, new_server_sender),
                                            },
                                    });

                                    let cancel = ObserverMessage {
                                        arena_id,
                                        body: ObserverMessageBody::<CommonRequest<G::GameRequest>, CommonUpdate<G::GameUpdate>>::Register {
                                            player_id,
                                            observer: server_sender.clone(),
                                            supports_unreliable: Self::SUPPORTS_UNRELIABLE,
                                        },
                                    };

                                    //if rand::random() {
                                    //    server.do_send(cancel);
                                    //    continue;
                                    //}

                                    let Ok(query) = serde_urlencoded::from_str(&query_string) else {
                                        server.do_send(cancel);
                                        continue;
                                    };
                                    let client_auth_request =
                                        ClientAuthRequest::new::<G>(query, this.as_ref().addr().ip(), origin.clone(), user_agent_id);

                                    let result = match server.send(client_auth_request).await {
                                        Ok(Ok(ok)) => ok,
                                        Ok(Err(e)) => {
                                            log::error!("redial auth failed: {e:?}");
                                            server.do_send(cancel);
                                            continue;
                                        }
                                        Err(e) => {
                                            log::error!("redial failed: {e}");
                                            server.do_send(cancel);
                                            continue;
                                        }
                                    };


                                    let old_arena_id = arena_id;
                                    let old_player_id = player_id;

                                    arena_id = result.0;
                                    player_id = result.1;

                                    server.do_send(ObserverMessage {
                                        arena_id,
                                        body: ObserverMessageBody::<CommonRequest<G::GameRequest>, CommonUpdate<G::GameUpdate>>::Register {
                                            player_id,
                                            observer: server_sender.clone(),
                                            supports_unreliable: Self::SUPPORTS_UNRELIABLE,
                                        },
                                    });

                                    warn!("redial {old_arena_id:?}/{old_player_id:?} -> {arena_id:?}/{player_id:?}");
                                }
                                Ok(request) => {
                                    server.do_send(ObserverMessage{
                                        arena_id,
                                        body: ObserverMessageBody::<CommonRequest<G::GameRequest>, CommonUpdate<G::GameUpdate >>::Request {
                                            player_id,
                                            request,
                                        }
                                    });
                                }
                                Err(err) => {
                                    if let Some(new) = warnings_left.checked_sub(1) {
                                        warnings_left = new;
                                        use base64::prelude::*;
                                        let len = message.as_ref().len();
                                        let snippet = BASE64_STANDARD_NO_PAD.encode(&message.as_ref()[0..len.min(256)]);
                                        warn!("{} sent invalid binary {err} (reliable={reliable}) {len}B \"{snippet}\"", this.as_ref().addr().ip());
                                    }
                                }
                            }
                        }
                        SocketMessage::Close{error} => {
                            info!("received close from client: error={error}");
                            break CLOSE_SILENT;
                        }
                    }
                },
                maybe_observer_update = server_receiver.recv() => {
                    let observer_update = match maybe_observer_update {
                        Some(observer_update) => observer_update,
                        None => {
                            // infrastructure wants websocket closed.
                            warn!("dropping socket");
                            break CLOSE_OK
                        }
                    };
                    match observer_update {
                        ObserverUpdate::Send{message, reliable} => {
                            let bytes = encode_buffer(&message);
                            let size = bytes.len();
                            let bytes = Bytes::from(bytes);
                            let socket_message = if reliable {
                                SocketMessage::Reliable(bytes)
                            } else {
                                SocketMessage::Unreliable(bytes)
                            };
                            if let Err(e) = this.as_mut().send(socket_message).await {
                                warn!("closing after failed to send {size} bytes: {e}");
                                break CLOSE_ERROR;
                            }
                        }
                        ObserverUpdate::Close => {
                            info!("closing socket");
                            break CLOSE_OK;
                        }
                    }
                },
            }
        };

        server.do_send(
            ObserverMessage {
                arena_id,
                body: ObserverMessageBody::<
                    CommonRequest<G::GameRequest>,
                    CommonUpdate<G::GameUpdate>,
                >::Unregister {
                    player_id,
                    observer: server_sender,
                },
            },
        );
        if let Some(error) = result {
            let _ = this.as_mut().send(SocketMessage::Close { error }).await;
        }
    }
}
