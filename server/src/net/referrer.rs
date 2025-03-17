// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::Referrer;
use axum::extract::FromRequestParts;
use axum_extra::TypedHeader;
use std::convert::Infallible;
use std::marker::PhantomData;

use crate::ArenaService;

// TODO: was pub(crate)
#[derive(Debug)]
pub struct ExtractReferrer<G>(pub(crate) Option<Referrer>, pub(crate) PhantomData<G>);

impl<G: ArenaService, S> FromRequestParts<S> for ExtractReferrer<G>
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let uri = parts.uri.authority().and_then(|authority| {
            Referrer::from_hostname(authority.host(), G::GAME_CONSTANTS.domain)
        });
        let origin = TypedHeader::<axum_extra::headers::Origin>::from_request_parts(parts, state)
            .await
            .ok()
            .and_then(|origin| {
                Referrer::from_hostname(origin.hostname(), G::GAME_CONSTANTS.domain)
            });
        let host = TypedHeader::<axum_extra::headers::Host>::from_request_parts(parts, state)
            .await
            .ok()
            .and_then(|host| Referrer::from_hostname(host.hostname(), G::GAME_CONSTANTS.domain));
        Ok(Self(uri.or(host).or(origin), PhantomData))
    }
}
