//! Simple key-value cache with TTL support.

use super::{CacheError, RedisClient};
use fred::interfaces::LuaInterface;
use fred::prelude::*;
use fred::types::scan::Scanner;
use futures::TryStreamExt;
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;

impl RedisClient {
    /// Get a cached value by key.
    ///
    /// Returns `None` if the key doesn't exist or has expired.
    pub async fn cache_get<T: DeserializeOwned>(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<T>, CacheError> {
        let full_key = self.make_key(namespace, key);
        let result: Option<String> = self
            .client
            .get(&full_key)
            .await
            .map_err(CacheError::CommandFailed)?;

        match result {
            Some(json) => {
                let value: T = serde_json::from_str(&json)
                    .map_err(|e| CacheError::SerializationFailed(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Set a cached value with TTL.
    ///
    /// The value is serialized as JSON and stored with the given TTL.
    pub async fn cache_set<T: Serialize>(
        &self,
        namespace: &str,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        let full_key = self.make_key(namespace, key);
        let json = serde_json::to_string(value)
            .map_err(|e| CacheError::SerializationFailed(e.to_string()))?;

        self.client
            .set::<(), _, _>(
                &full_key,
                json,
                Some(Expiration::EX(ttl.as_secs() as i64)),
                None,
                false,
            )
            .await
            .map_err(CacheError::CommandFailed)?;

        Ok(())
    }

    /// Delete a cached key.
    pub async fn cache_delete(&self, namespace: &str, key: &str) -> Result<(), CacheError> {
        let full_key = self.make_key(namespace, key);
        self.client
            .del::<(), _>(&full_key)
            .await
            .map_err(CacheError::CommandFailed)?;
        Ok(())
    }

    /// Delete all keys matching a pattern.
    ///
    /// Uses SCAN to iterate and delete. Not atomic but safe for cleanup.
    pub async fn cache_invalidate_pattern(
        &self,
        namespace: &str,
        pattern: &str,
    ) -> Result<u64, CacheError> {
        let full_pattern = self.make_key(namespace, pattern);
        let mut count: u64 = 0;
        let mut scan_stream = self.client.scan(&full_pattern, Some(100), None);

        while let Some(mut page) = scan_stream
            .try_next()
            .await
            .map_err(CacheError::CommandFailed)?
        {
            if let Some(keys) = page.take_results()
                && !keys.is_empty()
            {
                let deleted = keys.len() as u64;
                self.client
                    .del::<(), _>(keys)
                    .await
                    .map_err(CacheError::CommandFailed)?;
                count += deleted;
            }
            page.next();
        }

        Ok(count)
    }

    /// Atomically read and delete a refresh token.
    ///
    /// Uses a Lua script to guarantee that concurrent refresh requests
    /// cannot both read the same token before it's deleted.
    /// Returns the user_id if the token existed, None otherwise.
    pub async fn refresh_token_rotate(&self, token: &str) -> Result<Option<String>, CacheError> {
        let full_key = self.make_key("refresh", token);

        // Lua: atomic GET + DEL — prevents token family leaks under concurrency
        let script = r#"
            local val = redis.call('GET', KEYS[1])
            if val then
                redis.call('DEL', KEYS[1])
                return val
            end
            return ''
        "#;

        let keys = vec![full_key];
        let args: Vec<String> = vec![];
        let result: String = self
            .client
            .eval(script, keys, args)
            .await
            .map_err(CacheError::CommandFailed)?;

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestValue {
        id: u64,
        name: String,
    }

    #[test]
    fn cache_set_serializes_value() {
        let val = TestValue {
            id: 42,
            name: "test".to_string(),
        };
        let json = serde_json::to_string(&val).unwrap();
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"name\":\"test\""));
    }

    #[test]
    fn cache_get_deserializes_value() {
        let json = r#"{"id":1,"name":"hello"}"#;
        let val: TestValue = serde_json::from_str(json).unwrap();
        assert_eq!(val.id, 1);
        assert_eq!(val.name, "hello");
    }

    #[test]
    fn cache_get_deserializes_none_on_empty() {
        let result: Option<TestValue> = None;
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn integration_cache_get_hit() {
        let client = RedisClient::new("redis://127.0.0.1:6379/9", "test")
            .await
            .expect("redis connect");
        client
            .cache_set("test_ns", "hit_key", &"hit_value", Duration::from_secs(60))
            .await
            .unwrap();
        let result: Option<String> = client.cache_get("test_ns", "hit_key").await.unwrap();
        assert_eq!(result, Some("hit_value".to_string()));
        client.cache_delete("test_ns", "hit_key").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn integration_cache_get_miss() {
        let client = RedisClient::new("redis://127.0.0.1:6379/9", "test")
            .await
            .expect("redis connect");
        let result: Option<String> = client.cache_get("test_ns", "nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn integration_cache_delete() {
        let client = RedisClient::new("redis://127.0.0.1:6379/9", "test")
            .await
            .expect("redis connect");
        client
            .cache_set("test_ns", "del_key", &"to_delete", Duration::from_secs(60))
            .await
            .unwrap();
        client.cache_delete("test_ns", "del_key").await.unwrap();
        let result: Option<String> = client.cache_get("test_ns", "del_key").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn integration_cache_invalidate_pattern() {
        let client = RedisClient::new("redis://127.0.0.1:6379/9", "test")
            .await
            .expect("redis connect");
        client
            .cache_set("pat", "k1", &1u64, Duration::from_secs(60))
            .await
            .unwrap();
        client
            .cache_set("pat", "k2", &2u64, Duration::from_secs(60))
            .await
            .unwrap();
        let count = client.cache_invalidate_pattern("pat", "*").await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn integration_refresh_token_rotate() {
        let client = RedisClient::new("redis://127.0.0.1:6379/9", "test")
            .await
            .expect("redis connect");
        let token = "rotate-test-token";
        client
            .cache_set("refresh", token, &"user-123", Duration::from_secs(60))
            .await
            .unwrap();
        let result = client.refresh_token_rotate(token).await.unwrap();
        assert_eq!(result, Some("user-123".to_string()));
        let second = client.refresh_token_rotate(token).await.unwrap();
        assert!(second.is_none(), "second read after rotate should be None");
    }
}
