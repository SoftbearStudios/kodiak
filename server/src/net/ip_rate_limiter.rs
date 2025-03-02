// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::entry_point::HTTP_RATE_LIMITER;
use crate::rate_limiter::{RateLimiterProps, RateLimiterState, Units};
use kodiak_common::rand::random;
use log::warn;
use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Div;
use std::time::{Duration, Instant};

/// Helps limit the rate that a particular IP can perform an action.
#[derive(Debug)]
pub struct IpRateLimiter {
    usage: HashMap<IpAddr, Usage>,
    props: RateLimiterProps,
    next_prune: Instant,
    warning_limiter: RateLimiterState,
}

impl IpRateLimiter {
    pub(crate) fn connections_actives_per_ip(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.usage.values().map(|v| (v.connections, v.active))
    }

    pub(crate) fn should_limit_rate_outer(
        ip: IpAddr,
        usage: Units,
        label: &str,
        now: Instant,
    ) -> bool {
        let mut this = HTTP_RATE_LIMITER.lock().unwrap();
        this.should_limit_rate_inner(ip, usage, label, now)
    }

    pub(crate) fn should_limit_rate_inner(
        &mut self,
        ip: IpAddr,
        usage: Units,
        label: &str,
        now: Instant,
    ) -> bool {
        let should_rate_limit = self.should_limit_rate_with_usage_and_now(ip, usage, now);
        let should_warn = should_rate_limit
            && !self
                .warning_limiter
                .should_limit_rate_with_now(&WARNING_LIMIT, now);
        if should_warn {
            warn!("Bandwidth limiting {label} for {ip}");
        }
        should_rate_limit
    }
}

#[derive(Default, Debug)]
struct Usage {
    rate_limit: RateLimiterState,
    connections: u32,
    active: u32,
}

const WARNING_LIMIT: RateLimiterProps = RateLimiterProps::const_new(Duration::from_millis(100), 3);

#[derive(Debug)]
pub struct ConnectionPermit(IpAddr);

impl ConnectionPermit {
    pub(crate) fn new(ip: IpAddr, label: &str) -> Option<Self> {
        let now = Instant::now();
        let mut limiter = HTTP_RATE_LIMITER.lock().unwrap();
        let limiter = &mut *limiter;
        if limiter.should_limit_rate_inner(ip, 10000, label, now) {
            return None;
        }
        let entry = limiter.usage.entry(ip).or_default();
        let soft_limit = entry.active * 3 + 12;
        let hard_limit = soft_limit + 3;
        if entry.connections >= hard_limit || (entry.connections >= soft_limit && random()) {
            if !limiter
                .warning_limiter
                .should_limit_rate_with_now(&WARNING_LIMIT, now)
            {
                warn!(
                    "Count limiting {label} for {ip} ({} conn, {} active)",
                    entry.connections, entry.active
                );
            }
            None
        } else {
            entry.connections += 1;
            Some(Self(ip))
        }
    }
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        if let Some(usage) = HTTP_RATE_LIMITER.lock().unwrap().usage.get_mut(&self.0) {
            debug_assert!(usage.connections > 0);
            usage.connections = usage.connections.saturating_sub(1);
        } else {
            debug_assert!(false);
        }
    }
}

#[derive(Debug)]
pub struct ActivePermit(IpAddr);

impl ActivePermit {
    pub(crate) fn new(addr: IpAddr) -> Self {
        HTTP_RATE_LIMITER
            .lock()
            .unwrap()
            .usage
            .entry(addr)
            .or_default()
            .active += 1;
        Self(addr)
    }
}

impl Drop for ActivePermit {
    fn drop(&mut self) {
        if let Some(usage) = HTTP_RATE_LIMITER.lock().unwrap().usage.get_mut(&self.0) {
            debug_assert!(usage.active > 0);
            usage.active = usage.active.saturating_sub(1);
        } else {
            debug_assert!(false);
        }
    }
}

impl IpRateLimiter {
    /// Rate limit must be at least one millisecond.
    /// Burst must be less than the max value of the datatype.
    pub fn new(rate_limit: Duration, burst: Units) -> Self {
        Self::from(RateLimiterProps::new(rate_limit, burst))
    }

    /// Uses [`Units`] to represent bytes, to limit bandwidth.
    pub fn new_bandwidth_limiter(bytes_per_second: Units, bytes_burst: Units) -> Self {
        let rate_limit = Duration::from_secs(1).div(bytes_per_second);
        Self::new(rate_limit, bytes_burst)
    }

    /// Marks the action as being performed by the ip address.
    /// Returns true if the action should be blocked (rate limited).
    pub fn should_limit_rate(&mut self, ip: IpAddr) -> bool {
        self.should_limit_rate_with_usage(ip, 1)
    }

    pub fn should_limit_rate_with_usage(&mut self, ip: IpAddr, usage: Units) -> bool {
        self.should_limit_rate_with_usage_and_now(ip, usage, Instant::now())
    }

    /// Marks usage as being performed by the ip address.
    /// Returns true if the action should be blocked (rate limited).
    pub fn should_limit_rate_with_usage_and_now(
        &mut self,
        ip: IpAddr,
        usage: Units,
        now: Instant,
    ) -> bool {
        let should_limit_rate = self
            .usage
            .entry(ip)
            .or_insert(Usage {
                rate_limit: RateLimiterState {
                    until: now,
                    burst_used: 0,
                },
                connections: 0,
                active: 0,
            })
            .rate_limit
            .should_limit_rate_with_now_and_usage(&self.props, now, usage);

        self.maybe_prune(now);

        should_limit_rate
    }

    /// Clean up old items. Called automatically, not it is not necessary to call manually.
    fn maybe_prune(&mut self, now: Instant) {
        if now < self.next_prune {
            return;
        }
        self.next_prune = now + Duration::from_secs(5);
        self.prune(now);
    }

    fn prune(&mut self, now: Instant) {
        self.usage.retain(|_, usage: &mut Usage| {
            usage.rate_limit.until > now || usage.active > 0 || usage.connections > 0
        })
    }

    /// Returns size of internal data-structure.
    #[allow(clippy::len_without_is_empty)]
    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.usage.len()
    }
}

impl From<RateLimiterProps> for IpRateLimiter {
    fn from(props: RateLimiterProps) -> Self {
        Self {
            usage: HashMap::new(),
            props,
            next_prune: Instant::now(),
            warning_limiter: Default::default(),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::ip_rate_limiter::IpRateLimiter;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::time::Duration;

    #[test]
    pub fn ip_rate_limiter() {
        let ip_one = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let ip_two = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        let mut limiter = IpRateLimiter::new(Duration::from_millis(100), 3);

        assert_eq!(limiter.len(), 0);
        assert!(!limiter.should_limit_rate(ip_one));
        assert_eq!(limiter.len(), 1);
        assert!(!limiter.should_limit_rate(ip_one));
        assert_eq!(limiter.len(), 1);

        limiter.maybe_prune();
        assert_eq!(limiter.len(), 1);

        assert!(!limiter.should_limit_rate(ip_one));
        assert_eq!(limiter.len(), 1);

        limiter.maybe_prune();
        assert_eq!(limiter.len(), 1);

        assert!(limiter.should_limit_rate(ip_one));
        assert_eq!(limiter.len(), 1);

        std::thread::sleep(Duration::from_millis(250));

        assert!(!limiter.should_limit_rate(ip_two));
        assert_eq!(limiter.len(), 2);
        assert!(!limiter.should_limit_rate(ip_two));
        assert_eq!(limiter.len(), 2);

        limiter.maybe_prune();
        assert_eq!(limiter.len(), 2);

        std::thread::sleep(Duration::from_millis(100));

        limiter.maybe_prune();
        assert_eq!(limiter.len(), 1);

        std::thread::sleep(Duration::from_millis(500));

        limiter.maybe_prune();
        assert_eq!(limiter.len(), 0);

        assert!(!limiter.should_limit_rate(ip_one));
        assert_eq!(limiter.len(), 1);
    }
}
