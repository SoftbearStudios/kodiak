// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::actor::ServerActor;
use super::entry_point::{Authenticated, CORS_ALTERNATIVE_DOMAINS, REDIRECT_TO_SERVER_ID};
use super::net::{limit_content_length, IpRateLimiter, KillSwitch};
use super::rate_limiter::{RateLimiterProps, RateLimiterState};
use super::service::ArenaService;
use super::socket::ws_request;
use super::state::AppState;
use crate::files::{
    ads_txt_file, related_website_json, robots_txt_file, sitemap_txt_file, system_json_file,
    translation_json_file, StaticFilesHandler,
};
use crate::{AdminRequest, Referrer, ServerId, ServerNumber};
use actix::Addr;
use axum::body::{Body, HttpBody};
use axum::extract::{ConnectInfo, State};
use axum::http::uri::{Authority, Scheme};
use axum::http::{HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use bytes::Bytes;
use hyper::header::{CACHE_CONTROL, CONNECTION, CONTENT_LENGTH};
use kodiak_common::DomainName;
use minicdn::MiniCdn;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

pub async fn admin_request<G: ArenaService>(
    State(state): State<AppState<G>>,
    _: Authenticated,
    request: Json<AdminRequest>,
) -> impl IntoResponse {
    match state.server.send(request.0).await {
        Ok(result) => match result {
            Ok(update) => Ok(Json(update)),
            Err(e) => Err((StatusCode::BAD_REQUEST, String::from(e)).into_response()),
        },
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
    }
}

pub fn new_router<G: ArenaService>(
    server_id: ServerId,
    infrastructure: Addr<ServerActor<G>>,
    game_client: Arc<RwLock<MiniCdn>>,
    ads_txt: Arc<RwLock<HashMap<Option<Referrer>, Bytes>>>,
) -> Router {
    let cors_layer = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            move |origin, _parts| {
                origin
                    .to_str()
                    .map(|o| check_origin::<G>(o).is_some())
                    .unwrap_or(false)
            },
        ))
        .allow_headers(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::HEAD, Method::POST, Method::OPTIONS]);

    let state = AppState::<G>::new(server_id, infrastructure, ads_txt);
    Router::new()
        .fallback_service(get(StaticFilesHandler {
            cdn: game_client,
            prefix: "",
            browser_router: true,
        }))
        .route("/system.json", get(system_json_file))
        .route("/translation.json", get(translation_json_file))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            redirect_middleware::<G>,
        ))
        // Outside redirection because:
        // - WebSockets can't be redirected by standard HTTP redirect response
        // - Want to allow reconnection and travel to closed servers
        // - Closing servers can block new players later on
        // - Need that anyway, as WebTransport is not subject to the router
        .route("/ws", any(ws_request))
        // Need both, see https://github.com/tokio-rs/axum/issues/1607#issuecomment-1335025399
        .route("/admin/", post(admin_request))
        .route("/admin/{*path}", post(admin_request))
        .route("/ads.txt", get(ads_txt_file))
        .route("/robots.txt", get(robots_txt_file::<G>))
        .route("/sitemap.txt", get(sitemap_txt_file::<G>))
        .route(
            "/.well-known/related-website-set.json",
            get(related_website_json),
        )
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(cors_layer)
                .layer(axum::middleware::from_fn(security_middleware)),
        )
        // We limit even further later on.
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(axum_server_timing::ServerTimingLayer::new("Router"))
}

#[derive(Clone, Debug)]
pub enum AllowedOrigin {
    /// Primary game domain or softbear domain, or localhost.
    Primary,
    AlternativeDomain(DomainName),
}

impl AllowedOrigin {
    #[allow(unused)]
    pub(crate) fn is_alternative_domain(&self) -> bool {
        matches!(self, Self::AlternativeDomain(_))
    }

    pub(crate) fn alternative_domain(&self) -> Option<DomainName> {
        if let Self::AlternativeDomain(domain) = self {
            Some(*domain)
        } else {
            None
        }
    }
}

pub fn check_origin<G: ArenaService>(origin: &str) -> Option<AllowedOrigin> {
    let result = check_origin_quiet::<G>(origin);
    if result.is_none() {
        static WARN: LazyLock<Mutex<RateLimiterState>> = LazyLock::new(|| Mutex::default());
        if !WARN
            .lock()
            .unwrap()
            .should_limit_rate(&RateLimiterProps::new_pure(Duration::from_secs(1)))
        {
            log::warn!("CORS error: {origin}");
        }
    }
    result
}

fn check_origin_quiet<G: ArenaService>(origin: &str) -> Option<AllowedOrigin> {
    if false && cfg!(debug_assertions) {
        Some(AllowedOrigin::Primary)
    } else {
        let origin = origin
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        if origin
            .rsplit_once(':')
            .filter(|(l, p)| {
                let Ok(port) = u16::from_str(p) else {
                    return false;
                };
                l.ends_with("localhost") || l.ends_with("127.0.0.1") || port == 8080 || port == 8443
            })
            .is_some()
        {
            return Some(AllowedOrigin::Primary);
        }

        fn check(domain: &str, origin: &str) -> bool {
            if let Some(prefix) = origin.strip_suffix(domain) {
                if prefix.is_empty()
                    || prefix.ends_with('.')
                    || domain.bytes().filter(|b| *b == b'.').count() >= 2
                {
                    return true;
                }
            }
            false
        }

        let aliases = CORS_ALTERNATIVE_DOMAINS.lock().unwrap();
        for domain in [G::GAME_CONSTANTS.domain, "softbear.com"] {
            if check(domain, origin) {
                return Some(AllowedOrigin::Primary);
            }
        }
        for domain in aliases.iter() {
            if check(domain, origin) {
                return Some(AllowedOrigin::AlternativeDomain(*domain));
            }
        }

        None
    }
}

async fn security_middleware(
    request: axum::http::Request<Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let addr = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0);
    let ip = addr.map(|addr| addr.ip());

    const BASE: u32 = 1000;
    let version = request.version();
    use axum::http::Version;
    let label = if request.headers().get(CONNECTION).map(|v| v.as_bytes()) == Some(b"upgrade") {
        "UPGRADE request"
    } else {
        match version {
            Version::HTTP_09 => "HTTP/0.9 request",
            Version::HTTP_10 => "HTTP/1 request",
            Version::HTTP_11 => "HTTP/1.1 request",
            Version::HTTP_2 => "HTTP/2 request",
            _ => "? request",
        }
    };
    let h1 = [axum::http::Version::HTTP_10, axum::http::Version::HTTP_11].contains(&version);
    if let Some(ip) = ip
        && IpRateLimiter::should_limit_rate_outer(ip, BASE, label, Instant::now())
    {
        let mut builder = Response::builder().status(StatusCode::TOO_MANY_REQUESTS);

        // If they have to make a new connection, the firewall will get angry at them.
        if h1 {
            builder = builder.header(CONNECTION, "close");
        } else if let Some(kill_switch) = request.extensions().get::<KillSwitch>() {
            // Blow up the entire HTTP/2 connection (no better option).
            kill_switch.kill()
        }
        return Err(builder.body(Body::empty()).unwrap());
    }

    if !request
        .headers()
        .get("auth")
        .and_then(|hv| hv.to_str().ok())
        .map(Authenticated::validate)
        .unwrap_or(false)
    {
        #[allow(clippy::question_mark)] // Breaks type inference on Ok.
        if let Err(response) = limit_content_length(request.headers(), 16384) {
            return Err(response);
        }
    }

    let mut response = next.run(request).await;

    // Add some universal default headers.
    for (key, value) in [(CACHE_CONTROL, "no-cache")]
        .into_iter()
        .chain(h1.then(|| (HeaderName::from_static("keep-alive"), "timeout=5")))
    {
        response
            .headers_mut()
            .entry(key)
            .or_insert_with(|| HeaderValue::from_static(value));
    }

    let content_length = response.body().size_hint().lower() as u32;

    if let Some(ip) = ip
        && cfg!(not(debug_assertions))
        && content_length > BASE
        && IpRateLimiter::should_limit_rate_outer(
            ip,
            content_length,
            "HTTP response",
            Instant::now(),
        )
    {
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        response.headers_mut().remove(CONTENT_LENGTH);

        // I changed my mind, I'm not actually going to send you all this data...
        response = response.map(|_| Body::empty());
    }

    Ok(response)
}

async fn redirect_middleware<G: ArenaService>(
    State(state): State<AppState<G>>,
    request: axum::http::Request<Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let raw_path = request.uri().path();
    // The unwrap_or is purely defensive and should never happen.
    let path = raw_path.split('#').next().unwrap_or(raw_path);

    // Note: index.html redirects cause a visible disruption in web browsers.
    //
    // We want to redirect everything except index.html (at any path level) so the
    // browser url-bar remains intact.
    let redirect = !path.is_empty() && !path.ends_with('/');

    if redirect {
        let redirect_server_number = ServerNumber::new(
            REDIRECT_TO_SERVER_ID.load(Ordering::Relaxed),
        )
        .filter(|&server_number| {
            server_number != state.server_id.number || state.server_id.kind.is_local()
        });

        if let Some(server_number) = redirect_server_number {
            let scheme = request.uri().scheme().cloned().unwrap_or(Scheme::HTTPS);
            if let Ok(authority) = Authority::from_str(&format!(
                "{}.{}",
                server_number.0.get(),
                G::GAME_CONSTANTS.domain
            )) {
                let mut builder = Uri::builder().scheme(scheme).authority(authority);

                if let Some(path_and_query) = request.uri().path_and_query() {
                    builder = builder.path_and_query(path_and_query.clone());
                }

                if let Ok(uri) = builder.build() {
                    return Err(Redirect::temporary(&uri.to_string()));
                }
            }
        }
    }

    Ok(next.run(request).await)
}
