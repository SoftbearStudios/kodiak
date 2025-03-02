// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

mod acceptor;
mod http;
mod ip;
mod ip_rate_limiter;
mod referrer;
mod tls;
mod user_agent;
mod web_socket;

pub use self::acceptor::{CustomAcceptor, KillSwitch};
pub use self::http::limit_content_length;
pub use self::ip::{get_own_public_ip, ip_to_region_id};
pub use self::ip_rate_limiter::{ActivePermit, ConnectionPermit, IpRateLimiter};
pub use self::referrer::ExtractReferrer;
pub use self::tls::load_domains;
pub use self::user_agent::user_agent_into_id;
pub use self::web_socket::WebSocket;
