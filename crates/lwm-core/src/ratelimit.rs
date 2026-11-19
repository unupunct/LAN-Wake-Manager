use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Sliding-window limiter keyed by an arbitrary string (usually client IP + bucket).
#[derive(Default)]
pub struct RateLimiter {
    map: Mutex<HashMap<String, Vec<Instant>>>,
}
impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }
    /// Err(retry_after_secs) when over the limit.
    pub fn check(&self, key: &str, limit: usize, window: Duration) -> Result<(), u64> {
        let now = Instant::now();
        let mut m = self.map.lock().unwrap();
        if m.len() > 10_000 {
            m.retain(|_, v| v.last().map(|t| now.duration_since(*t) < window).unwrap_or(false));
        }
        let v = m.entry(key.to_string()).or_default();
        v.retain(|t| now.duration_since(*t) < window);
        if v.len() >= limit {
            let wait = window.saturating_sub(now.duration_since(v[0])).as_secs() + 1;
            return Err(wait);
        }
        v.push(now);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn limits() {
        let r = RateLimiter::new();
        for _ in 0..3 {
            assert!(r.check("a", 3, Duration::from_secs(60)).is_ok());
        }
        assert!(r.check("a", 3, Duration::from_secs(60)).is_err());
        assert!(r.check("b", 3, Duration::from_secs(60)).is_ok());
    }
}
