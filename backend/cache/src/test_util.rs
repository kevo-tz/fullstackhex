/// Test helpers for Redis integration tests.
///
/// Provides [`require_redis_url`] and [`TEST_NAMESPACE`] used by both cache and pubsub test modules.
use super::RedisClient;

pub const TEST_NAMESPACE: &str = "test";

/// Read `REDIS_URL` from environment. Prints skip message and returns empty
/// string when unset, so callers can early-return without false test failure.
pub fn require_redis_url() -> String {
    match std::env::var("REDIS_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("=== SKIP: REDIS_URL not set (Redis integration test) ===");
            String::new()
        }
    }
}

/// Create a [`RedisClient`] for integration testing.
///
/// Returns `None` when `url` is empty (i.e. `REDIS_URL` was not set),
/// so tests can early-return without false failure.
pub async fn test_client(url: &str, namespace: &str) -> Option<RedisClient> {
    if url.is_empty() {
        return None;
    }
    Some(
        RedisClient::new(url, namespace)
            .await
            .expect("redis connect"),
    )
}
