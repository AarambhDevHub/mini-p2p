use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// A Token Bucket Rate Limiter
///
/// This struct limits the rate of operations (e.g., bytes transferred)
/// by maintaining a "bucket" of tokens. Tokens are added at a fixed rate,
/// and operations must acquire tokens to proceed.
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    rate_per_sec: u64,
    capacity: u64,
    tokens: f64,
    last_update: Instant,
}

impl RateLimiter {
    /// Create a new RateLimiter
    ///
    /// # Arguments
    /// * `rate_per_sec` - The maximum number of tokens (bytes) allowed per second.
    /// * `capacity` - The maximum burst size (bucket size).
    pub fn new(rate_per_sec: u64, capacity: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                rate_per_sec,
                capacity,
                tokens: capacity as f64,
                last_update: Instant::now(),
            })),
        }
    }

    /// Acquire a specific number of tokens.
    ///
    /// If there are enough tokens, this returns immediately.
    /// If not, it waits until enough tokens are available.
    pub async fn acquire(&self, amount: u64) {
        if amount == 0 {
            return;
        }

        loop {
            let wait_duration = {
                let mut inner = self.inner.lock().await;
                inner.refill();

                if inner.tokens >= amount as f64 {
                    inner.tokens -= amount as f64;
                    return;
                } else {
                    // Calculate how long to wait for enough tokens
                    let missing = amount as f64 - inner.tokens;
                    let wait_secs = missing / inner.rate_per_sec as f64;
                    Duration::from_secs_f64(wait_secs)
                }
            };

            sleep(wait_duration).await;
        }
    }
}

impl Inner {
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        let new_tokens = elapsed * self.rate_per_sec as f64;

        if new_tokens > 0.0 {
            self.tokens = (self.tokens + new_tokens).min(self.capacity as f64);
            self.last_update = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        let rate = 100; // 100 bytes per second
        let limiter = RateLimiter::new(rate, rate);

        let start = Instant::now();

        // Consume all tokens immediately (burst)
        limiter.acquire(100).await;
        assert!(start.elapsed().as_millis() < 10); // Should be instant

        // Consume another 100, should take ~1 second
        limiter.acquire(100).await;
        assert!(start.elapsed().as_millis() >= 1000);
    }
}
