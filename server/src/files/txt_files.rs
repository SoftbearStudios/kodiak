// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::net::ExtractReferrer;
use crate::service::ArenaService;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub async fn ads_txt_file<G: ArenaService>(
    State(state): State<AppState<G>>,
    ExtractReferrer(referrer, _): ExtractReferrer<G>,
) -> impl IntoResponse {
    let ads_txts = state.ads_txt.read().unwrap();
    let ads_txt = ads_txts.get(&referrer).or(ads_txts.get(&None));
    if let Some(ads_txt) = ads_txt {
        Ok(plain_txt_file(Body::from(ads_txt.clone()), 3600))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub fn plain_txt_file(text: Body, ttl_secs: usize) -> Response {
    let cache_control = format!("public, max-age={ttl_secs}");
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .header("Cache-Control", cache_control)
        .body(text)
        .unwrap()
        .into_response()
}

pub async fn robots_txt_file<G: ArenaService>() -> Response {
    let d = G::GAME_CONSTANTS.domain;
    plain_txt_file(
        Body::from(format!(
            "User-agent: *\nAllow: /\nSitemap: https://{d}/sitemap.txt\n"
        )),
        60,
    )
}

pub async fn sitemap_txt_file<G: ArenaService>() -> Response {
    let d = G::GAME_CONSTANTS.domain;
    plain_txt_file(
        Body::from(format!("https://{d}/\nhttps://{d}/help/\nhttps://{d}/about/\nhttps://{d}/privacy/\nhttps://{d}/terms/\n")),
        60,
    )
}
