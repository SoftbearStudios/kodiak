// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{KEEPALIVE_HARD_TIMEOUT, KEEPALIVE_INTERVAL};
use crate::actor::{ClientAuthErr, ClientAuthRequest, ServerActor};
use crate::net::ConnectionPermit;
use crate::rate_limiter::RateLimiter;
use crate::router::check_origin;
use crate::socket::{Socket, SocketMessage, INBOUND_HARD_LIMIT};
use crate::ArenaService;
use actix::Addr;
use axum_server::tls_rustls::RustlsConfig;
use bytes::BytesMut;
use kodiak_common::rand::{thread_rng, RngCore};
use kodiak_common::{Compression, CompressionImpl, Compressor, SocketQuery};
use quinn::crypto::HandshakeTokenKey;
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::io::{self, ErrorKind};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use wtransport::config::QuicTransportConfig;
use wtransport::error::{SendDatagramError, StreamWriteError};
use wtransport::proto::WEBTRANSPORT_ALPN;
use wtransport::{quinn, Connection, Endpoint, RecvStream, SendStream, VarInt};

#[pin_project::pin_project]
pub struct WebTransportSocket {
    connection: Connection,
    send: SendStream,
    recv: RecvStream,
    recv_buffer: BytesMut,
    compressor: <CompressionImpl as Compression>::Compressor,
}

type Size = u32;

pub enum SendError {
    Stream(StreamWriteError),
    Datagram(SendDatagramError),
}

impl Error for SendError {}

impl Debug for SendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stream(stream) => Debug::fmt(stream, f),
            Self::Datagram(datagram) => Debug::fmt(datagram, f),
        }
    }
}

impl Display for SendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stream(stream) => Display::fmt(stream, f),
            Self::Datagram(datagram) => Display::fmt(datagram, f),
        }
    }
}

impl Socket for WebTransportSocket {
    type RecvErr = io::Error;
    type SendErr = SendError;

    const SUPPORTS_UNRELIABLE: bool = true;

    /// If `reliable` is `false`, the receiver is allowed to miss it or get it out of order.
    async fn send(mut self: Pin<&mut Self>, message: SocketMessage) -> Result<(), Self::SendErr> {
        match message {
            SocketMessage::Unreliable(message)
                if self
                    .as_ref()
                    .connection
                    .max_datagram_size()
                    .map(|max| message.len() <= max)
                    .unwrap_or(false) =>
            {
                self.as_mut()
                    .connection
                    .send_datagram(CompressionImpl::compress(&message))
                    .map_err(SendError::Datagram)
            }
            SocketMessage::Reliable(message) | SocketMessage::Unreliable(message) => {
                let mut message = self.compressor.compress(&message);
                message.splice(..0, (message.len() as Size).to_be_bytes());
                self.as_mut()
                    .send
                    .write_all(&message)
                    .await
                    .map_err(SendError::Stream)
            }
            SocketMessage::Close { error } => {
                self.as_mut()
                    .send
                    .finish()
                    .await
                    .map_err(SendError::Stream)?;
                //self.send.reset(VarInt::from_u32(error as u32));
                self.as_mut()
                    .connection
                    .close(VarInt::from_u32(error as u32), &[]);
                Ok(())
            }
        }
    }

    async fn recv(self: Pin<&mut Self>) -> Result<SocketMessage, Self::RecvErr> {
        let this = self.project();
        loop {
            if let Some(size_bytes) = this.recv_buffer.array_chunks().next() {
                let size = Size::from_be_bytes(*size_bytes) as usize;
                if size > INBOUND_HARD_LIMIT {
                    return Err(io::Error::new(ErrorKind::Other, "too big"));
                }
                if size + std::mem::size_of::<Size>() <= this.recv_buffer.len() {
                    let remaining = this
                        .recv_buffer
                        .split_off(size + std::mem::size_of::<Size>());
                    let mut front = std::mem::replace(this.recv_buffer, remaining);
                    let payload = front.split_off(std::mem::size_of::<Size>());
                    return Ok(SocketMessage::Reliable(payload.freeze()));
                }
            }

            tokio::select! {
                result = this.recv.read_buf(this.recv_buffer) => {
                    if result? == 0 {
                        return Err(io::Error::new(ErrorKind::UnexpectedEof, "EOF"));
                    }
                }
                result = this.connection.receive_datagram() => {
                    return if let Ok(d) = &result && d.len() > INBOUND_HARD_LIMIT {
                        Err(io::Error::new(ErrorKind::Other, "too big"))
                    } else {
                        result
                            .map(|d| {
                                SocketMessage::Unreliable(d.payload())
                            })
                            .map_err(|e| io::Error::new(ErrorKind::Other, e.to_string()))
                    };
                }
            }
        }
    }

    fn rtt(&self) -> Option<Duration> {
        Some(self.connection.rtt())
    }

    fn addr(&self) -> SocketAddr {
        canonize(self.connection.remote_address())
    }
}

fn canonize(addr: SocketAddr) -> SocketAddr {
    if let SocketAddr::V6(v6) = addr
        && let Some(v4) = v6.ip().to_ipv4_mapped()
    {
        // `quinn` converts IPv4 to IPv6, undo that.
        SocketAddr::V4(SocketAddrV4::new(v4, v6.port()))
    } else if let SocketAddr::V6(v6) = addr
        && v6.ip().is_loopback()
    {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, v6.port()))
    } else {
        addr
    }
}

fn generate_token_key() -> Arc<dyn HandshakeTokenKey> {
    let rng = &mut thread_rng();
    let mut master_key = [0u8; 64];
    rng.fill_bytes(&mut master_key);
    let master_key = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, &[]).extract(&master_key);
    Arc::new(master_key)
}

pub async fn web_transport<G: ArenaService>(
    server: Addr<ServerActor<G>>,
    port: u16,
    rustls_config: RustlsConfig,
) -> std::io::Result<()> {
    let token_key = generate_token_key();
    let config = new_config(port, &rustls_config, token_key.clone());
    let endpoint = Endpoint::server(config)?;
    let mut reload_rate_limit = RateLimiter::new(Duration::from_secs(60), 0);

    loop {
        if !reload_rate_limit.should_limit_rate() {
            // Turns out this is infallible if rebind=false.
            endpoint.reload_config(new_config(port, &rustls_config, token_key.clone()), false)?;
        }

        let incoming_session = endpoint.accept().await;

        let open = endpoint.open_connections();
        if open > 1000 {
            incoming_session.refuse();
            continue;
        } else if
        /* open > 250  &&*/
        !incoming_session.remote_address_validated() {
            incoming_session.retry();
            continue;
        }

        let ip = canonize(incoming_session.remote_address()).ip();
        let Some(permit) = ConnectionPermit::new(ip, "QUIC connection") else {
            incoming_session.refuse();
            continue;
        };

        let server = server.clone();
        tokio::spawn(async move {
            let incoming_request = incoming_session
                .await
                .map_err(|e| io::Error::new(ErrorKind::Other, e.to_string()))?;
            let Some(origin) = incoming_request.origin().and_then(|o| check_origin::<G>(o)) else {
                incoming_request.forbidden().await;
                return Err(io::Error::new(ErrorKind::Other, "invalid origin"));
            };

            let query_string = incoming_request
                .path()
                .split_once('?')
                .map(|(_, q)| q)
                .unwrap_or("");
            let query: SocketQuery = serde_urlencoded::from_str(query_string)
                .map_err(|e| io::Error::new(ErrorKind::Other, e.to_string()))?;
            let user_agent: Option<&str> = incoming_request.user_agent();
            let ip = canonize(incoming_request.remote_address()).ip();

            let user_agent_id = user_agent
                .or(query.user_agent.as_deref())
                .and_then(|h| crate::net::user_agent_into_id(h));
            let client_auth_request =
                ClientAuthRequest::new::<G>(query, ip, origin.clone(), user_agent_id);
            let result = server
                .send(client_auth_request)
                .await
                .map_err(|e| io::Error::new(ErrorKind::Other, e.to_string()))?;
            let (arena_id, player_id) = match result {
                Ok(ok) => ok,
                Err(e) => {
                    if matches!(e, ClientAuthErr::TooManyRequests) {
                        incoming_request.too_many_requests().await;
                    } else {
                        incoming_request.forbidden().await;
                    }
                    return Err(io::Error::new(ErrorKind::Other, {
                        let e: &'static str = e.into();
                        e
                    }));
                }
            };

            let connection = incoming_request
                .accept()
                .await
                .map_err(|e| io::Error::new(ErrorKind::Other, e.to_string()))?;
            let (send, recv) = connection
                .accept_bi()
                .await
                .map_err(|e| io::Error::new(ErrorKind::Other, e.to_string()))?;

            let socket = std::pin::pin!(WebTransportSocket {
                connection,
                send,
                recv,
                recv_buffer: Default::default(),
                compressor: Default::default(),
            });
            socket
                .serve(origin, user_agent_id, arena_id, player_id, server)
                .await;
            drop(permit);
            std::io::Result::Ok(())
        });
    }
}

fn new_config(
    port: u16,
    rustls_config: &RustlsConfig,
    token_key: Arc<dyn HandshakeTokenKey>,
) -> wtransport::ServerConfig {
    let mut rustls_config = rustls_config.get_inner().deref().clone();
    rustls_config.alpn_protocols = vec![WEBTRANSPORT_ALPN.to_vec()];

    let mut transport = QuicTransportConfig::default();

    transport.max_concurrent_uni_streams(4u32.into());
    transport.max_concurrent_bidi_streams(4u32.into());
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(KEEPALIVE_HARD_TIMEOUT).unwrap(),
    ));
    transport.keep_alive_interval(Some(KEEPALIVE_INTERVAL));

    let crypto: Arc<quinn::crypto::rustls::QuicServerConfig> = Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(rustls_config)
            .expect("CipherSuite::TLS13_AES_128_GCM_SHA256 missing"),
    );
    let mut server_config = quinn::ServerConfig::new(crypto, token_key);
    server_config.transport_config(Arc::new(transport));
    server_config.migration(true);
    server_config.max_incoming(256);
    server_config.incoming_buffer_size(256 * 1024);
    server_config.incoming_buffer_size_total(8 * 1024 * 1024);
    server_config.retry_token_lifetime(Duration::from_secs(10));

    wtransport::ServerConfig::builder()
        .with_bind_default(port)
        .build_with_quic_config(server_config)
}
