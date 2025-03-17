// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

/// For rate-limiting tasks on the client (where durations are expressed in seconds).
#[derive(Debug)]
pub struct RateLimiter {
    elapsed: f32,
    period: f32,
}

impl RateLimiter {
    pub fn new(period: f32) -> Self {
        Self {
            elapsed: period,
            period,
        }
    }

    /// Fast tracks the next update to return true.
    pub fn fast_track(&mut self) {
        self.elapsed = self.period;
    }

    /// Cancels any progress towards the next update.
    pub fn dismiss(&mut self) {
        self.elapsed = 0.0;
    }

    /// Reset the period of a rate limiter.
    pub fn set_period(&mut self, period: f32) {
        self.period = period;
    }

    /// Takes how much time passed, in seconds, since last update.
    pub fn update(&mut self, elapsed: f32) {
        debug_assert!(elapsed >= 0.0);
        self.elapsed += elapsed;
    }

    /// Returns whether it is time to do the rate limited action, clearing the elapsed time.
    pub fn ready(&mut self) -> bool {
        let ret = self.elapsed >= self.period;
        if ret {
            self.elapsed = 0.0;
        }
        ret
    }

    /// Takes how much time passed, in seconds, since last update. Returns whether it is time to
    /// do the rate-limited action.
    pub fn update_ready(&mut self, elapsed: f32) -> bool {
        self.update(elapsed);
        let ret = self.elapsed >= self.period;
        self.elapsed %= self.period; // TODO explain why this differs from ready which sets it to 0.
        ret
    }

    /// Takes how much time passed, in seconds, since last update. Returns a iterator of possibly
    /// multiple times to do the rate-limited action. Useful if called less frequently than period.
    /// Will iterate up to a second worth of updates max.
    pub fn iter_updates(&mut self, elapsed: f32) -> impl Iterator<Item = ()> {
        self.update(elapsed);
        let iterations = (self.elapsed / self.period) as usize;
        self.elapsed -= iterations as f32 * self.period;
        std::iter::repeat(()).take(iterations.min((1.0 / self.period) as usize))
    }
}

/// For rate-limiting idempotent values sent from the client to the server e.g. mouse position.
pub struct RateLimited<T> {
    rate_limiter: RateLimiter,
    t: Option<T>,
}

impl<T: PartialEq> RateLimited<T> {
    pub fn new(period: f32) -> Self {
        Self {
            rate_limiter: RateLimiter::new(period),
            t: None,
        }
    }

    /// Forgets the previous `t` but maintains the rate limit.
    pub fn clear(&mut self) {
        self.t = None;
    }

    /// Reduces the rate limit by `elapsed_seconds`. Returns true if the rate limit is over and `t` has changed since
    /// the last time the rate limit was over.
    pub fn tick(&mut self, elapsed_seconds: f32, t: T) -> bool {
        self.rate_limiter.update(elapsed_seconds);
        if !self.t.as_ref().is_some_and(|t2| t2 == &t) && self.rate_limiter.ready() {
            self.t = Some(t);
            true
        } else {
            false
        }
    }
}

impl<T: Copy + PartialEq> RateLimited<T> {
    /// Like [`Self::tick`] but calls `f` with `t` instead of returning true.
    pub fn tick_with(&mut self, elapsed_seconds: f32, t: T, f: impl FnOnce(T)) {
        if self.tick(elapsed_seconds, t) {
            f(self.t.unwrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_updates() {
        let mut limiter = RateLimiter::new(0.1);
        // Starts with 1 ready (see RateLimiter::new).
        assert!(limiter.update_ready(0.0));
        assert_eq!(limiter.iter_updates(10.0).count(), 10);

        assert!(!limiter.update_ready(0.06));
        assert!(limiter.update_ready(0.06));
        assert!(!limiter.update_ready(0.06));
        assert_eq!(limiter.iter_updates(0.15).count(), 2);
    }
}
