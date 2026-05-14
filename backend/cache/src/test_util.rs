/// Test helpers for Redis integration tests.
///
/// Provides [`require_redis_url`] used by both cache and pubsub test modules.

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
