// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

//! The game server has authority over all game logic. Clients are served the client, which connects
//! via web_socket.

use crate::actor::ServerActor;
use crate::cli::Options;
use crate::files::{set_open_file_limit, static_size_and_hash};
use crate::net::{get_own_public_ip, ip_to_region_id, load_domains, CustomAcceptor, IpRateLimiter};
use crate::rate_limiter::RateLimiterProps;
use crate::router::new_router;
use crate::service::ArenaService;
use crate::socket::web_transport;
use crate::{AdminRequest, AdminUpdate, DomainDto, RealmId, ServerId, ServerKind, ServerNumber};
use actix::Actor;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::headers::authorization::Bearer;
use axum_extra::headers::Authorization;
use axum_extra::TypedHeader;
use axum_server::accept::Accept;
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use futures::future::OptionFuture;
use kodiak_common::rand::{thread_rng, Rng};
use kodiak_common::DomainName;
use log::{error, info, warn};
use minicdn::MiniCdn;
use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::runtime::Builder;

/// 0 is no redirect.
pub static REDIRECT_TO_SERVER_ID: AtomicU8 = AtomicU8::new(0);

/// Admin password.
pub static SERVER_TOKEN: AtomicU64 = AtomicU64::new(0);

// Will be overwritten first thing.
pub static HTTP_RATE_LIMITER: LazyLock<Mutex<IpRateLimiter>> =
    LazyLock::new(|| Mutex::new(IpRateLimiter::new_bandwidth_limiter(1, 0)));

pub static CORS_ALTERNATIVE_DOMAINS: LazyLock<Mutex<Arc<[DomainName]>>> =
    LazyLock::new(|| Mutex::new(Vec::new().into()));

pub struct Authenticated;

impl Authenticated {
    pub(crate) fn validate(value: &str) -> bool {
        value
            .parse::<u64>()
            .map(|parsed| parsed != 0 && parsed == SERVER_TOKEN.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

pub enum AuthenticatedError {
    Missing,
    Invalid,
}

impl IntoResponse for AuthenticatedError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            match self {
                Self::Missing => "missing key",
                Self::Invalid => "invalid key",
            },
        )
            .into_response()
    }
}

impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
{
    type Rejection = AuthenticatedError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let bearer = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
            .await
            .map_err(|_| AuthenticatedError::Missing)?;
        if Self::validate(bearer.0.token()) {
            Ok(Self)
        } else {
            warn!(
                "invalid key {} (correct is {})",
                bearer.0.token(),
                SERVER_TOKEN.load(Ordering::Relaxed)
            );
            Err(AuthenticatedError::Invalid)
        }
    }
}

#[derive(Debug)]
struct ExtractRealmId(#[allow(unused)] RealmId);

enum ExtractRealmIdError {
    Invalid,
}

impl IntoResponse for ExtractRealmIdError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            match self {
                Self::Invalid => "invalid realm name",
            },
        )
            .into_response()
    }
}

impl<S> FromRequestParts<S> for ExtractRealmId
where
    S: Send + Sync,
{
    type Rejection = ExtractRealmIdError;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        return Err(ExtractRealmIdError::Invalid);
        /*
        let origin = TypedHeader::<axum::headers::Origin>::from_request_parts(parts, state)
            .await
            .ok()
            .and_then(|origin| Self::parse(origin.hostname()));
        let host = TypedHeader::<axum::headers::Host>::from_request_parts(parts, state)
            .await
            .ok()
            .and_then(|host| Self::parse(host.hostname()));
        if let Some(realm_id) = host.or(origin) {
            Ok(Self(realm_id))
        } else {
            Err(ExtractRealmIdError::Invalid)
        }
        */
    }
}

/// Default stack size is sufficient on most platforms.
#[cfg(not(windows))]
const STACK_SIZE: Option<usize> = None;
/// Need more stack to avoid overflow on Windows.
#[cfg(windows)]
const STACK_SIZE: Option<usize> = Some(12_000_000);

#[inline(always)]
fn with_stack_size<A: Send + 'static, R: Send + 'static>(
    f: impl FnOnce(A) -> R + Send + 'static,
    a: A,
) -> R {
    if let Some(stack_size) = STACK_SIZE {
        std::thread::Builder::new()
            .name(String::from("main_more_stack"))
            .stack_size(stack_size)
            .spawn(move || f(a))
            .expect("could not spawn new main thread with more stack")
            .join()
            .unwrap()
    } else {
        f(a)
    }
}

#[must_use]
pub fn entry_point<G: ArenaService>(game_client: MiniCdn) -> ExitCode
where
    <G as ArenaService>::GameUpdate: std::fmt::Debug,
{
    with_stack_size(entry_point_inner::<G>, game_client)
}

#[must_use]
fn entry_point_inner<G: ArenaService>(game_client: MiniCdn) -> ExitCode
where
    <G as ArenaService>::GameUpdate: std::fmt::Debug,
{
    std::env::set_var("RUST_BACKTRACE", "1");

    static BALLAST: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    *BALLAST.lock().unwrap() = vec![0; 2 << 18];
    std::alloc::set_alloc_error_hook(|layout| {
        // Attempt to free space for a backtrace.
        if let Ok(mut ballast) = BALLAST.try_lock() {
            std::mem::drop(std::mem::take(&mut *ballast));
        }
        // Want to see the backtrace for debugging.
        panic!("memory allocation of {} bytes failed", layout.size());
    });

    actix::System::with_tokio_rt(|| {
        let mut builder = Builder::new_current_thread();
        builder.enable_io();
        builder.enable_time();

        // Avoid stack overflow.
        if let Some(stack_size) = STACK_SIZE {
            builder.thread_stack_size(stack_size);
        }

        builder.build().expect("could not build tokio runtime")
    })
    .block_on(async move {
        let options = Options::parse();
        options.init_logger();

        match set_open_file_limit(16384) {
            Ok(limit) => info!("set open file limit to {}", limit),
            Err(e) => error!("could not set open file limit: {}", e),
        }

        let (http_port, https_port) = options.http_and_https_ports();
        let (static_size, static_hash) = static_size_and_hash(&game_client);
        let bandwidth_burst = options.bandwidth_burst(static_size);

        *HTTP_RATE_LIMITER.lock().unwrap() =
            IpRateLimiter::new_bandwidth_limiter(options.http_bandwidth_limit, bandwidth_burst);
        SERVER_TOKEN.store(
            if let Some(token) = options.server_token {
                token.0.get()
            } else {
                let random = thread_rng().gen::<u64>();

                fn load_token() -> Option<u64> {
                    #[allow(deprecated)]
                    let dir = std::env::home_dir()?;
                    let path = dir.join(".plasma_bearer_token");
                    let mut file = File::open(path).ok()?;
                    let mut buf = String::new();
                    file.read_to_string(&mut buf).ok();
                    let n = buf.trim().parse::<u64>().ok();
                    n.filter(|n| *n != 0)
                }

                if let Some(token) = load_token() {
                    (random / token * token).max(token)
                } else {
                    warn!("no plasma bearer token");
                    random.max(1)
                }
            },
            Ordering::Relaxed,
        );

        let server_id = options.server_id().unwrap_or_else(|| ServerId {
            number: ServerNumber(thread_rng().gen()),
            kind: ServerKind::Local,
        });
        let ipv4_address = if let Some(ip_address) = options.ipv4_address {
            Some(ip_address)
        } else {
            get_own_public_ip().await
        };
        let region_id = options
            .region_id
            .or_else(|| ipv4_address.and_then(|ip| ip_to_region_id(IpAddr::V4(ip))))
            .unwrap_or_default();

        let game_client = Arc::new(RwLock::new(game_client));
        let ads_txt = Arc::default();
        #[allow(deprecated)]
        let (certificate, private_key) = options
            .certificate_private_key_paths()
            .map(|(c, p)| {
                (
                    Cow::Owned(std::fs::read_to_string(c).unwrap()),
                    Cow::Owned(std::fs::read_to_string(p).unwrap()),
                )
            })
            .unwrap_or((
                Cow::Borrowed(include_str!("./net/certificate.pem")),
                Cow::Borrowed(include_str!("./net/private_key.pem")),
            ));
        let rustls_config = RustlsConfig::from_config(
            load_domains::<G>(&[DomainDto {
                domain: G::GAME_CONSTANTS.domain_name(),
                certificate: certificate.into(),
                private_key: private_key.into(),
            }])
            .unwrap()
            .0,
        );
        // Awaiting https://github.com/actix/actix-net/issues/588
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

        // For debug.
        /*
        std::thread::spawn(|| {
            //std::thread::sleep(Duration::from_secs(300));
            loop {
                {
                    let lim = HTTP_RATE_LIMITER.lock().unwrap();
                    println!("IP limiter: {lim:?}");
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        */

        let srv = ServerActor::<G>::start(
            ServerActor::new(
                server_id,
                &REDIRECT_TO_SERVER_ID,
                static_hash,
                region_id,
                options.bots,
                Arc::clone(&ads_txt),
                &SERVER_TOKEN,
                rustls_config.clone(),
                &*CORS_ALTERNATIVE_DOMAINS,
                Some(options.domain_backup.into()),
                RateLimiterProps::new(
                    Duration::from_secs(options.client_authenticate_rate_limit),
                    options.client_authenticate_burst,
                ),
                stop_tx,
            )
            .await,
        );

        // Manual profile.
        if options.cpu_profile {
            let srv = srv.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                info!("starting CPU profile");
                let future = srv.send(AdminRequest::RequestCpuProfile(10));
                let result = future.await;
                if let Ok(Ok(AdminUpdate::CpuProfileRequested(profile))) = result {
                    if let Ok(mut file) = File::create("/tmp/server_cpu_profile.xml") {
                        if file.write_all(profile.as_bytes()).is_ok() {
                            info!("saved CPU profile");
                        }
                    }
                }
            });
        }

        if options.heap_profile {
            let srv = srv.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                info!("starting heap profile");
                let future = srv.send(AdminRequest::RequestHeapProfile(10));
                let result = future.await;
                if let Ok(Ok(AdminUpdate::HeapProfileRequested(profile))) = result {
                    if let Ok(mut file) = File::create("/tmp/server_heap_profile.json") {
                        if file.write_all(profile.as_bytes()).is_ok() {
                            info!("saved heap profile");
                        }
                    }
                }
            });
        }

        let app = new_router(server_id, srv.clone(), game_client, ads_txt);

        #[cfg(not(debug_assertions))]
        let http_app = axum::Router::new().fallback_service(axum::routing::get(
            move |uri: axum::http::Uri,
                  host: TypedHeader<axum_extra::headers::Host>,
                  headers: reqwest::header::HeaderMap| async move {
                if let Err(response) = crate::net::limit_content_length(&headers, 16384) {
                    return Err(response);
                }

                use axum::http::uri::{Authority, Scheme};
                use axum::http::Uri;
                use axum::response::Redirect;
                use std::str::FromStr;

                let mut parts = uri.into_parts();
                parts.scheme = Some(Scheme::HTTPS);
                let authority = if https_port == Options::STANDARD_HTTPS_PORT {
                    Authority::from_str(host.0.hostname())
                } else {
                    // non-standard port.
                    Authority::from_str(&format!("{}:{}", host.0.hostname(), https_port))
                }
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
                parts.authority = Some(authority);
                Uri::from_parts(parts)
                    .map(|uri| {
                        if http_port == Options::STANDARD_HTTP_PORT {
                            Redirect::permanent(&uri.to_string())
                        } else {
                            Redirect::temporary(&uri.to_string())
                        }
                    })
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
            },
        ));

        #[cfg(debug_assertions)]
        let http_app = app.clone();

        trait ConfigureExt<S, A> {
            fn configure(self) -> axum_server::Server<CustomAcceptor<S, A>>;
        }
        impl<S, A: Accept<TcpStream, S>> ConfigureExt<S, A> for axum_server::Server<A> {
            fn configure(mut self) -> axum_server::Server<CustomAcceptor<S, A>> {
                let http = self.http_builder();
                // `header_read_timeout` applies to all requests.
                http.http1()
                    .timer(hyper_util::rt::TokioTimer::new())
                    .keep_alive(true)
                    .header_read_timeout(Duration::from_secs(5))
                    .max_buf_size(32768)
                    .http2()
                    .timer(hyper_util::rt::TokioTimer::new())
                    .enable_connect_protocol()
                    .keep_alive_interval(Duration::from_secs(300))
                    // Impossible to respond within 0ns, so `keep_alive_interval` effectively becomes idle timeout.
                    .keep_alive_timeout(Duration::ZERO)
                    .max_header_list_size(512 * 1024)
                    .max_send_buf_size(64 * 1024)
                    .max_concurrent_streams(16);
                self.map(CustomAcceptor::new)
            }
        }

        let http_server = axum_server::bind(SocketAddr::from(([0, 0, 0, 0], http_port)))
            .configure()
            .serve(http_app.into_make_service_with_connect_info::<SocketAddr>());

        let https_server = axum_server::bind_rustls(
            SocketAddr::from(([0, 0, 0, 0], https_port)),
            rustls_config.clone(),
        )
        .configure()
        .serve(app.into_make_service_with_connect_info::<SocketAddr>());

        let wt_server: OptionFuture<_> = if G::GAME_CONSTANTS.udp_enabled {
            Some(web_transport(srv.clone(), https_port, rustls_config))
        } else {
            None
        }
        .into();

        let mut exit_code = ExitCode::FAILURE;
        tokio::select! {
            result = http_server => {
                error!("http server stopped: {result:?}");
            }
            result = https_server => {
                error!("https server stopped: {result:?}");
            }
            result = wt_server, if G::GAME_CONSTANTS.udp_enabled => {
                error!("wt server stopped: {result:?}");
            }
            result = stop_rx => {
                if result.is_ok () {
                    error!("server actor stopped");
                } else {
                    error!("server actor dropped");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                error!("received Ctrl+C / SIGINT");
                exit_code = ExitCode::SUCCESS;
            }
        }

        srv.do_send(crate::shutdown::Shutdown);

        // Allow some time for the shutdown to propagate
        // but don't hang forever if it doesn't.
        tokio::time::sleep(Duration::from_millis(500)).await;

        exit_code
    })
}
