//! Rate limiting via Redis sorted sets (sliding window).
//!
//! Uses ZREMRANGEBYSCORE to clean old entries on each check,
//! keeping the sorted set bounded.

use super::{CacheError, RedisClient};
use fred::interfaces::LuaInterface;
use fred::prelude::*;
use fred::types::sorted_sets::ZRange;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
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
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
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
}
