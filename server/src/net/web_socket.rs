// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::acceptor::nodelay_keepalive;
use crate::{PlasmaRequest, PlasmaUpdate};
use actix::Recipient;
use axum::http::uri::InvalidUri;
use axum_tws::Config;
use futures::{SinkExt, StreamExt};
use hyper::Uri;
use log::{info, warn};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, ErrorKind};
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Sender};
use tokio_rustls::client::TlsStream;
use tokio_websockets::resolver::{self, Resolver};
use tokio_websockets::{
    ClientBuilder, Connector, Limits, MaybeTlsStream, Message, WebSocketStream,
};

// TODO: was pub(crate)
pub struct WebSocket {
    pub(crate) sender: Option<Sender<PlasmaRequest>>,
}

impl WebSocket {
    pub fn new() -> Self {
        Self { sender: None }
    }

    pub fn do_send(&self, message: PlasmaRequest) {
        if let Some(sender) = &self.sender {
            let _ = sender.try_send(message);
        }
    }

    pub fn spawn(&mut self, url: String, callback: Recipient<PlasmaUpdate>) {
        info!("connecting to {url:?}");
        let (sender, mut receiver) = channel(16);
        self.sender = Some(sender);
        tokio::spawn(async move {
            let mut connection: Option<WebSocketStream<TlsStream<TcpStream>>> = None;
            const TIMEOUT: Duration = Duration::from_secs(100);
            let mut timeout = std::pin::pin!(tokio::time::sleep(TIMEOUT));
            let mut tries = 0;
            loop {
                if let Some(websocket) = connection.as_mut() {
                    tokio::select! {
                        to_send = receiver.recv() => {
                            if let Some(to_send) = to_send {
                                let result = tokio::time::timeout(Duration::from_secs(16), websocket.send(Message::text(serde_json::to_string(&to_send).unwrap()))).await;
                                match result {
                                    Err(_) => {
                                        warn!("timed out while sending");
                                        connection = None;
                                    },
                                    Ok(Err(e)) => {
                                        warn!("failed to send {e}");
                                        connection = None;
                                    }
                                    Ok(Ok(_)) => {
                                        info!("sent {to_send:?}");
                                        let new_deadline: tokio::time::Instant = (Instant::now() + Duration::from_secs(7)).into();
                                        if new_deadline < timeout.as_mut().deadline() {
                                            // Since we expect a response soon, we can detect failures
                                            // faster than normal.
                                            timeout.as_mut().reset(new_deadline);
                                        }
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        received = websocket.next() => {
                            if let Some(Ok(received)) = received {
                                let string = if let Some(text) = received.as_text() {
                                    text
                                } else {
                                    continue;
                                };
                                timeout.as_mut().reset((Instant::now() + TIMEOUT).into());
                                if string.as_bytes().first() != Some(&b'{') {
                                    if !string.is_empty() {
                                        warn!("plasma: {string}");
                                    }
                                    continue;
                                }
                                match serde_json::from_str::<PlasmaUpdate>(string) {
                                    Ok(update) => {
                                        info!("received {update:?}");
                                        let _ = callback.try_send(update);
                                    }
                                    Err(e) => warn!("couldn't deserialize {string} due to {e:?}"),
                                }
                            } else {
                                warn!("failed to receive {received:?}");
                                connection = None;
                            }
                        }
                        _ = &mut timeout => {
                            warn!("timeout while receiving");
                            connection = None;
                        }
                    }
                } else {
                    tokio::time::sleep(Duration::from_secs(2u64.saturating_pow(tries).min(60)))
                        .await;
                    let result = Self::connect(url.clone()).await;
                    match result {
                        Ok(conn) => {
                            connection = Some(conn);
                            tries = 0;
                            timeout.as_mut().reset((Instant::now() + TIMEOUT).into());
                        }
                        Err(e) => {
                            warn!("failed to connect {e:?}");
                            tries += 1;
                        }
                    }
                }
            }
        });
    }

    async fn connect(url: String) -> Result<WebSocketStream<TlsStream<TcpStream>>, ConnectError> {
        let uri = Uri::from_str(&url).map_err(ConnectError::InvalidUri)?;

        let host = uri.host().ok_or(ConnectError::Other(
            tokio_websockets::Error::CannotResolveHost,
        ))?;
        let addr = resolver::Gai
            .resolve(host, 443)
            .await
            .map_err(ConnectError::Other)?;
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| ConnectError::Other(tokio_websockets::Error::Io(e)))?;
        nodelay_keepalive(&stream, 10, 4);
        let connector = Connector::new().map_err(ConnectError::Other)?;
        let stream = connector
            .wrap(host, stream)
            .await
            .map_err(ConnectError::Other)?;
        let MaybeTlsStream::Rustls(stream) = stream else {
            debug_assert!(false);
            return Err(ConnectError::Other(tokio_websockets::Error::Io(
                io::Error::new(ErrorKind::Other, "not client rustls"),
            )));
        };
        let result = tokio::time::timeout(
            Duration::from_secs(12),
            ClientBuilder::from_uri(uri)
                .config(Config::default().frame_size(32 * 1000))
                .limits(Limits::default().max_payload_len(Some(2usize.pow(21))))
                .connect_on(stream),
        )
        .await
        .map_err(|_| ConnectError::Timeout)
        .map(|result| {
            result
                .map(|(stream, _)| stream)
                .map_err(ConnectError::Other)
        })
        .flatten();
        if let Err(e) = &result {
            warn!("{e}");
        }
        result
    }
}

#[derive(Debug)]
enum ConnectError {
    InvalidUri(InvalidUri),
    Timeout,
    Other(tokio_websockets::Error),
}

impl Error for ConnectError {}

impl Display for ConnectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUri(e) => Display::fmt(e, f),
            Self::Timeout => f.write_str("timeout"),
            Self::Other(e) => Display::fmt(e, f),
        }
    }
}
