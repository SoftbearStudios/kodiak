// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::actor::SystemRequest;
use crate::net::ip_to_region_id;
use crate::service::ArenaService;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use axum_extra::TypedHeader;
use kodiak_common::{SystemQuery, TranslationRequest};
use std::net::SocketAddr;

pub async fn system_json_file<G: ArenaService>(
    state: State<AppState<G>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<SystemQuery>,
    user_agent: Option<TypedHeader<axum_extra::headers::UserAgent>>,
) -> impl IntoResponse {
    match state
        .server
        .send(SystemRequest {
            query,
            region_id: ip_to_region_id(addr.ip()),
            user_agent_id: user_agent
                .as_ref()
                .and_then(|u| crate::net::user_agent_into_id(u.as_str())),
        })
        .await
    {
        Ok(system_response) => Ok(Json(system_response)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
    }
}

pub async fn translation_json_file<G: ArenaService>(
    state: State<AppState<G>>,
    Query(request): Query<TranslationRequest>,
) -> impl IntoResponse {
    match state.server.send(request).await {
        Ok(translation_response) => Ok(Json(translation_response)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
    }
}

// /.well-known/related-website-set.json
pub async fn related_website_json<G: ArenaService>(
    _state: State<AppState<G>>,
) -> impl IntoResponse {
    r#"{"primary":"https://softbear.com"}"#
}
