//! Redis cache layer for FullStackHex.
//!
//! Provides caching, rate limiting, session management, and pub/sub
//! backed by Redis 8 via the fred crate.

use fred::prelude::*;
use std::time::Duration;

pub mod cache;
pub mod metrics;
pub mod pubsub;
pub mod rate_limit;
pub mod session;

/// Errors from the Redis cache layer.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("redis not configured")]
    NotConfigured,
    #[error("redis connection failed: {0}")]
    ConnectionFailed(String),
    #[error("redis command failed: {0}")]
    CommandFailed(#[from] fred::error::Error),
    #[error("serialization failed: {0}")]
    SerializationFailed(String),
    #[error("session not found")]
    SessionNotFound,
    #[error("rate limit exceeded")]
    RateLimitExceeded,
    #[error("backoff blocked: {count} failures, {label} cooldown, {remaining_secs}s remaining")]
    BackoffBlocked {
        remaining_secs: u64,
        count: u64,
        label: String,
    },
}

/// Redis client wrapper with connection pool.
pub struct RedisClient {
    client: Client,
    key_prefix: String,
}

impl RedisClient {
    /// Create a new Redis client from environment variables.
    ///
    /// Reads `REDIS_URL` for the connection string and `REDIS_POOL_SIZE`
    /// for the connection pool size (default: 10).
    pub async fn from_env() -> Result<Self, CacheError> {
        let redis_url = std::env::var("REDIS_URL").map_err(|_| CacheError::NotConfigured)?;

        let config = Config::from_url(&redis_url)
            .map_err(|e| CacheError::ConnectionFailed(e.to_string()))?;

        let client = Builder::from_config(config)
            .with_connection_config(|c| {
                c.connection_timeout = Duration::from_secs(5);
            })
            .with_performance_config(|c| {
                c.default_command_timeout = Duration::from_secs(5);
            })
            .set_policy(ReconnectPolicy::new_exponential(0, 100, 30_000, 2))
            .build()
            .map_err(|e| CacheError::ConnectionFailed(e.to_string()))?;

        client
            .init()
            .await
            .map_err(|e| CacheError::ConnectionFailed(e.to_string()))?;

        tracing::info!("redis connected");

        Ok(Self {
            client,
            key_prefix: "fullstackhex".to_string(),
        })
    }

    /// Create a Redis client for testing with explicit URL.
    pub async fn new(url: &str, prefix: &str) -> Result<Self, CacheError> {
        let config =
            Config::from_url(url).map_err(|e| CacheError::ConnectionFailed(e.to_string()))?;

        let client = Builder::from_config(config)
            .with_connection_config(|c| {
                c.connection_timeout = Duration::from_secs(5);
            })
            .build()
            .map_err(|e| CacheError::ConnectionFailed(e.to_string()))?;

        client
            .init()
            .await
            .map_err(|e| CacheError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            client,
            key_prefix: prefix.to_string(),
        })
    }

    /// Build a namespaced key: `{prefix}:{namespace}:{key}`
    pub fn make_key(&self, namespace: &str, key: &str) -> String {
        format!("{}:{}:{}", self.key_prefix, namespace, key)
    }

    /// Get the underlying fred client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Check if the Redis connection is healthy.
    pub async fn ping(&self) -> Result<(), CacheError> {
        let _: Value = self
            .client
            .ping(None)
            .await
            .map_err(CacheError::CommandFailed)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_error_display_variants() {
        assert_eq!(
            CacheError::NotConfigured.to_string(),
            "redis not configured"
        );
        assert!(
            CacheError::ConnectionFailed("err".to_string())
                .to_string()
                .contains("err")
        );
        assert!(
            CacheError::SerializationFailed("bad".to_string())
                .to_string()
                .contains("bad")
        );
        assert_eq!(CacheError::SessionNotFound.to_string(), "session not found");
        assert_eq!(
            CacheError::RateLimitExceeded.to_string(),
            "rate limit exceeded"
        );
    }

    #[test]
    fn cache_error_command_failed_from_fred_error() {
        let fred_err = fred::error::Error::new(fred::error::ErrorKind::IO, "test io error");
        let cache_err: CacheError = fred_err.into();
        assert!(matches!(cache_err, CacheError::CommandFailed(_)));
        assert!(cache_err.to_string().contains("test io error"));
    }

    #[test]
    fn cache_error_not_configured_display() {
        let err = CacheError::NotConfigured;
        assert_eq!(err.to_string(), "redis not configured");
    }

    #[test]
    fn cache_error_rate_limit_exceeded_display() {
        let err = CacheError::RateLimitExceeded;
        assert_eq!(err.to_string(), "rate limit exceeded");
    }

    #[tokio::test]
    async fn redis_client_new_invalid_url_fails() {
        let result = RedisClient::new("not-a-valid-url", "test").await;
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(matches!(err, CacheError::ConnectionFailed(_)));
        }
    }

    #[tokio::test]
    async fn redis_client_new_unreachable_url_fails() {
        // Valid-looking URL but unreachable host should fail at init stage
        let result = RedisClient::new("redis://invalid-host-test:1234/0", "test").await;
        assert!(result.is_err());
    }
}
