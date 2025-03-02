// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::fmt::{self, Debug, Formatter};
use std::time::{Duration, Instant};

/// A rate limiter that may have unique properties.
pub struct RateLimiter {
    props: RateLimiterProps,
    state: RateLimiterState,
}

/// A [`u32`] is chosen for being the widest type that doesn't increase the size of
/// [`RateLimiterState`] or [`RateLimiterProps`] on a 64-bit system, and it is natively accepted
/// by [`Duration::saturating_mul`].
pub type Units = u32;

/// The state of a rate limiter.
pub struct RateLimiterState {
    /// At this time, the limit is effectively brand new.
    pub(crate) until: Instant,
    pub burst_used: Units,
}

impl Debug for RateLimiterState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimiterState")
            .field(
                "until",
                &self
                    .until
                    .saturating_duration_since(Instant::now())
                    .as_secs_f32(),
            )
            .field("burst_used", &self.burst_used)
            .finish()
    }
}

/// The (sharable) properties of a rate limiter.
#[derive(Debug)]
pub struct RateLimiterProps {
    rate_limit: Duration,
    pub(crate) burst: Units,
}

impl RateLimiterState {
    /// Returns true if the action exceeds the rate limit defined by the props and should be prevented.
    pub fn should_limit_rate(&mut self, props: &RateLimiterProps) -> bool {
        self.should_limit_rate_with_now_and_usage(props, Instant::now(), 1)
    }

    /// Returns true if the action exceeds the rate limit defined by the props and should be prevented.
    #[allow(unused)]
    pub fn should_limit_rate_with_usage(&mut self, props: &RateLimiterProps, usage: Units) -> bool {
        self.should_limit_rate_with_now_and_usage(props, Instant::now(), usage)
    }

    /// Returns true if the action exceeds the rate limit defined by the props and should be prevented.
    pub fn should_limit_rate_with_now(&mut self, props: &RateLimiterProps, now: Instant) -> bool {
        self.should_limit_rate_with_now_and_usage(props, now, 1)
    }

    /// Like [`Self::should_rate_limit`] but more efficient if you already know the current time.
    pub fn should_limit_rate_with_now_and_usage(
        &mut self,
        props: &RateLimiterProps,
        now: Instant,
        usage: Units,
    ) -> bool {
        if props.rate_limit == Duration::ZERO {
            return false;
        }

        let ok = if now > self.until {
            self.burst_used = 0;
            true
        } else if self.burst_used.saturating_add(usage) <= props.burst {
            self.burst_used = self.burst_used.saturating_add(usage);
            true
        } else {
            false
        };

        if ok {
            if let Some(instant) = self
                .until
                .checked_add(props.rate_limit.saturating_mul(usage))
            {
                self.until = instant.max(now);
            }
        }

        !ok
    }
}

impl Default for RateLimiterState {
    fn default() -> Self {
        Self {
            until: Instant::now(),
            burst_used: 0,
        }
    }
}

impl RateLimiterProps {
    /// rate limit should be more than zero.
    /// burst must be less than [`Unit::MAX`], otherwise the limit is ineffectual.
    pub fn new(rate_limit: Duration, burst: Units) -> Self {
        debug_assert!(
            rate_limit != Duration::ZERO,
            "use RateLimiterProps::no_limit() to explicitly opt out of rate limiting"
        );
        debug_assert!(burst < Units::MAX);
        Self { rate_limit, burst }
    }

    /// rate limit should be more than zero.
    pub fn new_pure(rate_limit: Duration) -> Self {
        Self::new(rate_limit, 0)
    }

    /// Like [`new`] but const and no runtime checks are performed.
    pub const fn const_new(rate_limit: Duration, burst: Units) -> Self {
        Self { rate_limit, burst }
    }

    /// Properties of a rate limiter that allow infinite rate.
    pub fn no_limit() -> Self {
        Self {
            rate_limit: Duration::ZERO,
            burst: 0,
        }
    }
}

impl RateLimiter {
    /// Creates a new rate limiter with the specified properties.
    pub fn new(rate_limit: Duration, burst: Units) -> Self {
        Self::from(RateLimiterProps::new(rate_limit, burst))
    }

    /// Constructs a rate limiter that allows infinite rate.
    #[allow(unused)]
    pub fn no_limit() -> Self {
        Self::from(RateLimiterProps::no_limit())
    }

    /// Returns true if the action exceeds the rate limit and should be prevented.
    pub fn should_limit_rate(&mut self) -> bool {
        self.state.should_limit_rate(&self.props)
    }

    /// Like [`Self::should_limit_rate`] but more efficient if you already know the time.
    #[allow(unused)]
    pub fn should_limit_rate_with_now(&mut self, now: Instant) -> bool {
        self.state.should_limit_rate_with_now(&self.props, now)
    }
}

impl From<RateLimiterProps> for RateLimiter {
    fn from(props: RateLimiterProps) -> Self {
        Self {
            props,
            state: RateLimiterState::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::rate_limiter::RateLimiter;
    use std::time::Duration;

    #[test]
    fn fuzz() {
        let rate_limit = 80;
        let burst = 8;
        let mut rate_limiter = RateLimiter::new(Duration::from_millis(rate_limit), burst);
        let mut now = rate_limiter.state.until;

        for rate in 0..rate_limit + 50 {
            now += Duration::from_millis((burst as u64 + 1) * rate_limit);

            let mut total_ok = 0;
            for n in 0..burst * 2 {
                now += Duration::from_millis(rate);
                let ok = !rate_limiter.should_limit_rate_with_now(now);
                if ok {
                    total_ok += 1;
                } else {
                    assert!(n >= burst && rate <= rate_limit, "{n} {rate} {rate_limit}");
                }
            }
            assert!(total_ok >= burst);
            if rate > rate_limit {
                assert_eq!(rate_limiter.state.burst_used, 0, "{rate} {rate_limit}");
            }
        }
    }

    #[test]
    fn normal() {
        let mut rate_limiter = RateLimiter::new(Duration::from_millis(10), 2);

        for _ in 0..10 {
            assert!(!rate_limiter.should_limit_rate());
            assert_eq!(rate_limiter.state.burst_used, 0);
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    #[test]
    fn limit_exceeded() {
        let mut rate_limiter = RateLimiter::new(Duration::from_millis(10), 3);

        std::thread::sleep(Duration::from_millis(5));

        assert!(!rate_limiter.should_limit_rate());
        assert_eq!(rate_limiter.state.burst_used, 0);
        assert!(!rate_limiter.should_limit_rate());
        assert_eq!(rate_limiter.state.burst_used, 1);
        assert!(!rate_limiter.should_limit_rate());
        assert_eq!(rate_limiter.state.burst_used, 2);
        assert!(!rate_limiter.should_limit_rate());
        assert_eq!(rate_limiter.state.burst_used, 3);
        assert!(rate_limiter.should_limit_rate());
        assert_eq!(rate_limiter.state.burst_used, 3);

        std::thread::sleep(Duration::from_millis(50));

        assert!(!rate_limiter.should_limit_rate());
        assert_eq!(rate_limiter.state.burst_used, 0);
    }

    #[test]
    fn no_limit() {
        let mut rate_limiter = RateLimiter::no_limit();

        for _ in 0..1000 {
            assert!(!rate_limiter.should_limit_rate());
        }
    }
}
