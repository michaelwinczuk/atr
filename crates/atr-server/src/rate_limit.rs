//! Per-API-key rate limiting using a sliding window counter

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rate limiter with per-key sliding window counters
#[derive(Clone)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, WindowCounter>>>,
    default_limit: u32,
    window_duration: Duration,
}

struct WindowCounter {
    count: u32,
    window_start: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter.
    /// `default_limit`: max requests per window per key.
    /// `window_secs`: window duration in seconds.
    pub fn new(default_limit: u32, window_secs: u64) -> Self {
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
            default_limit,
            window_duration: Duration::from_secs(window_secs),
        }
    }

    /// Check if a request is allowed for the given API key.
    /// Returns `Ok(remaining)` if allowed, `Err(retry_after_secs)` if rate limited.
    pub fn check(&self, api_key: &str) -> Result<u32, u64> {
        let mut windows = self.windows.lock().unwrap();
        let now = Instant::now();

        let counter = windows
            .entry(api_key.to_string())
            .or_insert(WindowCounter {
                count: 0,
                window_start: now,
            });

        // Reset window if expired
        if now.duration_since(counter.window_start) >= self.window_duration {
            counter.count = 0;
            counter.window_start = now;
        }

        if counter.count >= self.default_limit {
            let elapsed = now.duration_since(counter.window_start);
            let retry_after = self
                .window_duration
                .checked_sub(elapsed)
                .unwrap_or(Duration::from_secs(1))
                .as_secs();
            Err(retry_after)
        } else {
            counter.count += 1;
            let remaining = self.default_limit - counter.count;
            Ok(remaining)
        }
    }

    /// Clean up expired windows to prevent memory leaks.
    /// Call periodically from a background task.
    pub fn cleanup(&self) {
        let mut windows = self.windows.lock().unwrap();
        let now = Instant::now();
        windows.retain(|_, counter| {
            now.duration_since(counter.window_start) < self.window_duration * 2
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(100, 60) // 100 requests per minute
    }
}
