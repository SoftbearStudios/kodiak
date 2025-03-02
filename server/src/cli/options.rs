// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{RegionId, ServerId, ServerKind, ServerToken};
use clap::Parser;
use log::LevelFilter;
use std::net::{Ipv4Addr, Ipv6Addr};

/// Server options, to be specified as arguments.
#[derive(Debug, Parser)]
#[clap(ignore_errors = true)]
pub struct Options {
    /// Override bot count to a constant.
    #[clap(long)]
    pub bots: Option<u16>,
    /// Log incoming HTTP requests
    #[cfg_attr(debug_assertions, clap(long, default_value = "warn"))]
    #[cfg_attr(not(debug_assertions), clap(long, default_value = "error"))]
    pub debug_http: LevelFilter,
    /// Log game diagnostics
    #[cfg_attr(debug_assertions, clap(long, default_value = "info"))]
    #[cfg_attr(not(debug_assertions), clap(long, default_value = "error"))]
    pub debug_game: LevelFilter,
    /// Log game engine diagnostics
    #[cfg_attr(debug_assertions, clap(long, default_value = "info"))]
    #[cfg_attr(not(debug_assertions), clap(long, default_value = "warn"))]
    pub debug_engine: LevelFilter,
    /// Log plasma diagnostics
    #[cfg_attr(debug_assertions, clap(long, default_value = "info"))]
    #[cfg_attr(not(debug_assertions), clap(long, default_value = "warn"))]
    pub debug_plasma: LevelFilter,
    #[clap(long, default_value = "./domain_backup.json")]
    pub domain_backup: String,
    /// Server ID.
    #[clap(long)]
    server_id: Option<ServerId>,
    /// Alternative to `server_id`.
    #[clap(long)]
    hostname: Option<String>,
    /// Initial secret key unique to this server.
    #[clap(long)]
    pub server_token: Option<ServerToken>,
    #[clap(long)]
    /// Override the server ipv4.
    pub ipv4_address: Option<Ipv4Addr>,
    #[clap(long)]
    /// Override the server ipv6 (currently unused).
    pub ipv6_address: Option<Ipv6Addr>,
    #[clap(long)]
    pub http_port: Option<u16>,
    #[clap(long)]
    pub https_port: Option<u16>,
    /// Override the region id.
    #[clap(long)]
    pub region_id: Option<RegionId>,
    /// Domain (without server id prepended).
    #[allow(dead_code)]
    #[deprecated = "now from game id"]
    #[clap(long)]
    pub domain: Option<String>,
    /// Certificate chain path.
    #[clap(long)]
    #[deprecated]
    pub certificate_path: Option<String>,
    /// Private key path.
    #[clap(long)]
    #[deprecated]
    pub private_key_path: Option<String>,
    /// HTTP request bandwidth limiting (in bytes per second).
    #[clap(long, default_value = "500000")]
    pub http_bandwidth_limit: u32,
    /// HTTP request rate limiting burst (in bytes).
    ///
    /// Implicit minimum is double the total size of the client static files.
    #[clap(long)]
    pub http_bandwidth_burst: Option<u32>,
    /// Client authenticate rate limiting period (in seconds).
    #[clap(long, default_value = "10")]
    pub client_authenticate_rate_limit: u64,
    /// Client authenticate rate limiting burst.
    #[clap(long, default_value = "16")]
    pub client_authenticate_burst: u32,
    #[clap(long)]
    pub cpu_profile: bool,
    #[clap(long)]
    pub heap_profile: bool,
}

impl Options {
    pub(crate) const STANDARD_HTTPS_PORT: u16 = 443;
    pub(crate) const STANDARD_HTTP_PORT: u16 = 80;

    #[deprecated]
    pub(crate) fn certificate_private_key_paths(&self) -> Option<(&str, &str)> {
        #[allow(deprecated)]
        self.certificate_path
            .as_deref()
            .zip(self.private_key_path.as_deref())
    }

    pub(crate) fn server_id(&self) -> Option<ServerId> {
        self.server_id.or_else(|| {
            self.hostname.as_ref().and_then(|hostname| {
                hostname
                    .split('.')
                    .next()
                    .unwrap()
                    .parse()
                    .ok()
                    .map(|number| ServerId {
                        number,
                        kind: ServerKind::Cloud,
                    })
            })
        })
    }

    pub(crate) fn bandwidth_burst(&self, static_size: usize) -> u32 {
        self.http_bandwidth_burst.unwrap_or(static_size as u32 * 2)
    }

    pub(crate) fn http_and_https_ports(&self) -> (u16, u16) {
        #[cfg(unix)]
        let priviledged = nix::unistd::Uid::effective().is_root();

        #[cfg(not(unix))]
        let priviledged = true;

        let (http_port, https_port) = if priviledged {
            (Self::STANDARD_HTTP_PORT, Self::STANDARD_HTTPS_PORT)
        } else {
            (8080, 8443)
        };

        let ports = (
            self.http_port.unwrap_or(http_port),
            self.https_port.unwrap_or(https_port),
        );
        log::info!("HTTP port: {}, HTTPS port: {}", ports.0, ports.1);
        ports
    }
}
