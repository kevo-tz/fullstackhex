//! Rate limiting via Redis sorted sets (sliding window).
//!
//! Uses ZREMRANGEBYSCORE to clean old entries on each check,
//! keeping the sorted set bounded.

use super::{CacheError, RedisClient};
use fred::interfaces::LuaInterface;
use fred::prelude::*;
use fred::types::sorted_sets::ZRange;
use std::time::Duration;

/// Returns the current time as ms since Unix epoch, or 0 if the system clock
/// is before the epoch (never panics).
fn unix_epoch_ms() -> u64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()) as u64
}

/// Rate limit check result.
pub struct RateLimitResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Number of requests remaining in the window.
    pub remaining: u64,
    /// When the rate limit resets (Unix timestamp ms).
    pub reset_at: u64,
}

impl RedisClient {
    /// Check and record a rate limit request.
    ///
    /// Uses a sliding window implemented via Redis sorted sets.
    /// Each request adds a member with the current timestamp as score.
    /// Old entries outside the window are cleaned up on each check.
    ///
    /// - `key`: unique key for this rate limit (e.g., "login:192.168.1.1")
    /// - `window`: time window duration
    /// - `max_requests`: maximum requests allowed in the window
    pub async fn rate_limit_check(
        &self,
        key: &str,
        window: Duration,
        max_requests: u64,
    ) -> Result<RateLimitResult, CacheError> {
        let full_key = self.make_key("ratelimit", key);
        let now_ms = unix_epoch_ms();
        let window_start = now_ms - window.as_millis() as u64;

        // Lua script: atomic check + add + cleanup
        // KEYS[1] = rate limit key
        // ARGV[1] = window start (ms)
        // ARGV[2] = current time (ms)
        // ARGV[3] = max requests
        // ARGV[4] = member ID (unique per request)
        // ARGV[5] = TTL for the key (seconds)
        let script = r#"
            redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1])
            local count = redis.call('ZCARD', KEYS[1])
            if count < tonumber(ARGV[3]) then
                redis.call('ZADD', KEYS[1], ARGV[2], ARGV[4])
                redis.call('EXPIRE', KEYS[1], ARGV[5])
                return {1, tonumber(ARGV[3]) - count - 1, 0}
            else
                local reset = redis.call('ZRANGE', KEYS[1], 0, 0, 'WITHSCORES')
                return {0, 0, tonumber(reset[2]) + tonumber(ARGV[5]) * 1000}
            end
        "#;

        let member_id = uuid::Uuid::new_v4().to_string();
        let ttl_secs = window.as_secs() + 60; // extra buffer for cleanup

        let keys = vec![full_key.clone()];
        let args = vec![
            window_start.to_string(),
            now_ms.to_string(),
            max_requests.to_string(),
            member_id,
            ttl_secs.to_string(),
        ];

        let result: Vec<i64> = self
            .client
            .eval(script, keys, args)
            .await
            .map_err(CacheError::CommandFailed)?;

        Ok(RateLimitResult {
            allowed: result[0] == 1,
            remaining: result[1].max(0) as u64,
            reset_at: result[2] as u64,
        })
    }

    /// Get the current rate limit count for a key (without incrementing).
    pub async fn rate_limit_count(&self, key: &str, window: Duration) -> Result<u64, CacheError> {
        let full_key = self.make_key("ratelimit", key);
        let now_ms = unix_epoch_ms();
        let window_start = now_ms - window.as_millis() as u64;

        // Clean old entries: remove all members with score <= window_start
        self.client
            .zremrangebyscore::<(), _, _, _>(
                &full_key,
                ZRange::from("-inf"),
                ZRange::from(window_start as i64),
            )
            .await
            .map_err(CacheError::CommandFailed)?;

        let count: u64 = self
            .client
            .zcard(&full_key)
            .await
            .map_err(CacheError::CommandFailed)?;

        Ok(count)
    }

    /// Check and apply progressive brute-force backoff.
    ///
    /// Tracks login failures per IP+endpoint. Failure count determines cooldown:
    ///   - 5 failures  → 60s block
    ///   - 10 failures → 5min block
    ///   - 20 failures → 30min block
    ///
    /// Returns an error with the remaining cooldown seconds if the IP is blocked.
    /// Call this BEFORE the rate limit check on login endpoints.
    pub async fn backoff_check(&self, ip: &str, endpoint: &str) -> Result<(), CacheError> {
        let key = self.make_key("backoff", &format!("{ip}:{endpoint}"));
        let count: Option<u64> = self
            .client
            .get::<Option<u64>, _>(&key)
            .await
            .map_err(CacheError::CommandFailed)?;

        let Some(count) = count else {
            return Ok(());
        };

        let (_ttl, label) = backoff_params(count);

        let remaining_ttl: i64 = self
            .client
            .ttl(&key)
            .await
            .map_err(CacheError::CommandFailed)?;

        if remaining_ttl > 0 {
            return Err(CacheError::BackoffBlocked {
                remaining_secs: remaining_ttl as u64,
                count,
                label: label.to_string(),
            });
        }

        // TTL expired — clean up the stale key
        if let Err(e) = self.client.del::<(), _>(&key).await {
            tracing::warn!(key = %key, error = %e, "backoff stale key cleanup failed");
        }
        Ok(())
    }

    /// Record a failed login attempt, incrementing the backoff counter.
    ///
    /// Call this AFTER a failed login (wrong password, invalid credentials).
    /// The TTL is set based on the current failure count threshold.
    pub async fn backoff_increment(&self, ip: &str, endpoint: &str) -> Result<(), CacheError> {
        let key = self.make_key("backoff", &format!("{ip}:{endpoint}"));

        let count: u64 = self
            .client
            .incr::<u64, _>(&key)
            .await
            .map_err(CacheError::CommandFailed)?;

        let (ttl_secs, _label) = backoff_params(count);

        self.client
            .expire::<(), _>(&key, ttl_secs as i64, None)
            .await
            .map_err(CacheError::CommandFailed)?;

        Ok(())
    }
}

/// Returns (ttl_seconds, label) for the given failure count.
fn backoff_params(count: u64) -> (u64, &'static str) {
    if count >= 20 {
        (1800, "30min") // 30 minutes
    } else if count >= 10 {
        (300, "5min") // 5 minutes
    } else if count >= 5 {
        (60, "60s") // 1 minute
    } else {
        (60, "tracking") // Track below threshold with 60s TTL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_below_threshold() {
        assert_eq!(backoff_params(0), (60, "tracking"));
        assert_eq!(backoff_params(4), (60, "tracking"));
    }

    #[test]
    fn backoff_level_1() {
        assert_eq!(backoff_params(5), (60, "60s"));
        assert_eq!(backoff_params(9), (60, "60s"));
    }

    #[test]
    fn backoff_level_2() {
        assert_eq!(backoff_params(10), (300, "5min"));
        assert_eq!(backoff_params(19), (300, "5min"));
    }

    #[test]
    fn backoff_level_3() {
        assert_eq!(backoff_params(20), (1800, "30min"));
        assert_eq!(backoff_params(200), (1800, "30min"));
    }
}
