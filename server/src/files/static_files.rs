// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

use crate::{ClientHash, CompatHasher};
use axum::body::Body;
use axum::handler::Handler;
use axum::http::header::{ACCEPT, ACCEPT_ENCODING, IF_NONE_MATCH};
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::response::Response;
use minicdn::{Base64Bytes, MiniCdn};
use std::borrow::Cow;
use std::future::ready;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct StaticFilesHandler {
    pub cdn: Arc<RwLock<MiniCdn>>,
    pub prefix: &'static str,
    pub browser_router: bool,
}

impl<S: Send + Sync + 'static> Handler<((),), S> for StaticFilesHandler {
    type Future = std::future::Ready<Response>;

    fn call(self, req: Request<Body>, _: S) -> Self::Future {
        // Path, minus preceding slash, prefix, and trailing index.html.
        let path = req
            .uri()
            .path()
            .trim_start_matches(self.prefix)
            .trim_start_matches('/')
            .trim_end_matches("index.html");

        let true_path = if self.browser_router && !path.contains('.') {
            // Browser routers require that all routes return the root index.html file.
            Cow::Borrowed("index.html")
        } else if path.is_empty() || path.ends_with('/') {
            // Undo removing index.html so we can lookup via rust_embed.
            Cow::Owned(format!("{}index.html", path))
        } else {
            Cow::Borrowed(path)
        };

        let files = self.cdn.read().unwrap();
        let file = match files.get(&true_path) {
            Some(file) => file,
            None => {
                return ready(
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Body::from("404 Not Found"))
                        .unwrap(),
                )
            }
        };

        let if_none_match = req.headers().get(IF_NONE_MATCH);

        let (accepting_brotli, accepting_gzip) = req
            .headers()
            .get(ACCEPT_ENCODING)
            .and_then(|h| h.to_str().ok())
            .map(|s| (s.contains("br"), s.contains("gzip")))
            .unwrap_or((false, false));

        let accepting_webp = req
            .headers()
            .get(ACCEPT)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.contains("image/webp"))
            .unwrap_or(false);

        let etag_matches = if_none_match.map_or(false, |inm| {
            let s: &str = file.etag.as_ref();
            inm == s
        });

        ready(if etag_matches {
            Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .body(Body::empty())
                .unwrap()
        } else if let Some(contents_webp) = file.contents_webp.as_ref().filter(|_| accepting_webp) {
            Response::builder()
                .header(header::ETAG, unsafe {
                    HeaderValue::from_maybe_shared_unchecked(file.etag.as_bytes().clone())
                })
                .header(header::CONTENT_TYPE, "image/webp")
                .body(Body::from(<Base64Bytes as Into<axum::body::Bytes>>::into(
                    contents_webp.clone(),
                )))
                .unwrap()
        } else if let Some(contents_brotli) =
            file.contents_brotli.as_ref().filter(|_| accepting_brotli)
        {
            Response::builder()
                .header(header::ETAG, unsafe {
                    HeaderValue::from_maybe_shared_unchecked(file.etag.as_bytes().clone())
                })
                .header(header::CONTENT_ENCODING, "br")
                .header(header::CONTENT_TYPE, unsafe {
                    HeaderValue::from_maybe_shared_unchecked(file.mime.as_bytes().clone())
                })
                .body(Body::from(<Base64Bytes as Into<axum::body::Bytes>>::into(
                    contents_brotli.clone(),
                )))
                .unwrap()
        } else if let Some(contents_gzip) = file.contents_gzip.as_ref().filter(|_| accepting_gzip) {
            Response::builder()
                .header(header::ETAG, unsafe {
                    HeaderValue::from_maybe_shared_unchecked(file.etag.as_bytes().clone())
                })
                .header(header::CONTENT_ENCODING, "gzip")
                .header(header::CONTENT_TYPE, unsafe {
                    HeaderValue::from_maybe_shared_unchecked(file.mime.as_bytes().clone())
                })
                .body(Body::from(<Base64Bytes as Into<axum::body::Bytes>>::into(
                    contents_gzip.clone(),
                )))
                .unwrap()
        } else {
            Response::builder()
                .header(header::ETAG, unsafe {
                    HeaderValue::from_maybe_shared_unchecked(file.etag.as_bytes().clone())
                })
                .header(header::CONTENT_TYPE, unsafe {
                    HeaderValue::from_maybe_shared_unchecked(file.mime.as_bytes().clone())
                })
                .body(Body::from(<Base64Bytes as Into<axum::body::Bytes>>::into(
                    file.contents.clone(),
                )))
                .unwrap()
        })
    }
}

/// Returns the size in bytes of all client files, followed by a collective hash of them.
pub fn static_size_and_hash(cdn: &MiniCdn) -> (usize, ClientHash) {
    let mut size = 0;
    let mut hash: u64 = 0;

    cdn.for_each(|path, file| {
        size += file.contents.len();
        let mut hasher = CompatHasher::default();
        path.hash(&mut hasher);
        file.etag.hash(&mut hasher);
        //println!("{:?} -> {}", path, hasher.finish());
        // Order-independent.
        hash ^= hasher.finish()
    });

    let hash = (hash >> 48) as u16 ^ (hash >> 32) as u16 ^ (hash >> 16) as u16 ^ hash as u16;

    (size, hash)
}
