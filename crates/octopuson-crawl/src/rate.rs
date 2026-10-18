//! A shared request rate limit.

use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// Spaces requests of all workers evenly to stay under a requests-per-second ceiling.
#[derive(Debug)]
pub(crate) struct RateLimiter {
    interval: Option<Duration>,
    /// None means the next slot lies beyond the monotonic clock's range.
    next: Mutex<Option<Instant>>,
}

impl RateLimiter {
    /// A validated finite, nonnegative ceiling; zero disables the limit.
    pub(crate) fn new(per_second: f64) -> Self {
        let interval = (per_second > 0.0).then(|| {
            // A very small positive rate can exceed Duration's range or even make its
            // reciprocal infinite. It still permits the first slot, with all later
            // slots beyond the clock's horizon rather than a panic or an unlimited rate.
            Duration::try_from_secs_f64(1.0 / per_second).unwrap_or(Duration::MAX)
        });
        Self {
            interval,
            next: Mutex::new(Some(Instant::now())),
        }
    }

    /// Waits for a browser resource slot without exceeding its render budget.
    pub(crate) fn wait_until(&self, deadline: Instant, control: &crate::CrawlControl) -> bool {
        if control.is_stopped() || Instant::now() >= deadline {
            return false;
        }
        let Some(interval) = self.interval else {
            return true;
        };
        let slot = {
            let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(next_slot) = *next else {
                return false;
            };
            let slot = next_slot.max(Instant::now());
            if slot >= deadline {
                return false;
            }
            *next = slot.checked_add(interval);
            slot
        };
        while Instant::now() < slot {
            if control.is_stopped() || Instant::now() >= deadline {
                return false;
            }
            thread::sleep(
                slot.saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(50)),
            );
        }
        !control.is_stopped() && Instant::now() < deadline
    }

    /// Waits for the next HTTP slot; false means Stop cancelled unsent work.
    pub(crate) fn wait(&self, control: &crate::CrawlControl) -> bool {
        if control.is_stopped() {
            return false;
        }
        let Some(interval) = self.interval else {
            return true;
        };
        let slot = {
            let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
            let now = Instant::now();
            let slot = (*next).map(|next| next.max(now));
            *next = slot.and_then(|slot| slot.checked_add(interval));
            slot
        };
        while slot.is_none_or(|slot| Instant::now() < slot) {
            if control.is_stopped() {
                return false;
            }
            thread::sleep(slot.map_or(Duration::from_millis(50), |slot| {
                slot.saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(50))
            }));
        }
        !control.is_stopped()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_wait_respects_budget_and_stop() {
        let limiter = RateLimiter::new(1.0);
        let control = crate::CrawlControl::new();
        assert!(limiter.wait_until(Instant::now() + Duration::from_millis(30), &control));
        let started = Instant::now();
        assert!(!limiter.wait_until(started + Duration::from_millis(30), &control));
        assert!(started.elapsed() < Duration::from_millis(200));
        control.stop();
        assert!(
            !RateLimiter::new(0.0).wait_until(Instant::now() + Duration::from_secs(1), &control)
        );
    }

    #[test]
    fn spaces_requests() {
        let limiter = RateLimiter::new(50.0);
        let control = crate::CrawlControl::new();
        let start = Instant::now();
        for _ in 0..4 {
            assert!(limiter.wait(&control));
        }
        // Three intervals of 20 ms after the first request.
        assert!(start.elapsed() >= Duration::from_millis(55));
        let unlimited = RateLimiter::new(0.0);
        let start = Instant::now();
        for _ in 0..100 {
            assert!(unlimited.wait(&control));
        }
        assert!(start.elapsed() < Duration::from_millis(50));
        control.stop();
        assert!(!unlimited.wait(&control));
        assert!(!limiter.wait(&control));
    }

    #[test]
    fn extreme_positive_rates_keep_http_cancellable_and_browser_budget_bounded() {
        for rate in [f64::from_bits(1), f64::MIN_POSITIVE, 1e-18, 1e-9] {
            let limiter = RateLimiter::new(rate);
            let control = crate::CrawlControl::new();
            assert!(limiter.wait(&control));
            let started = Instant::now();
            thread::scope(|scope| {
                scope.spawn(|| {
                    thread::sleep(Duration::from_millis(20));
                    control.stop();
                });
                assert!(!limiter.wait(&control));
            });
            assert!(started.elapsed() < Duration::from_millis(500));

            let browser = RateLimiter::new(rate);
            let control = crate::CrawlControl::new();
            let deadline = Instant::now() + Duration::from_millis(100);
            assert!(browser.wait_until(deadline, &control));
            assert!(!browser.wait_until(deadline, &control));
        }
        let limiter = RateLimiter::new(f64::MAX);
        let control = crate::CrawlControl::new();
        assert!(limiter.wait(&control));
        assert!(limiter.wait(&control));
    }
}
