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
        let redis_url =
            std::env::var("REDIS_URL").map_err(|_| CacheError::NotConfigured)?;

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
        let config = Config::from_url(url)
            .map_err(|e| CacheError::ConnectionFailed(e.to_string()))?;

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
