//! Simple key-value cache with TTL support.

use super::{CacheError, RedisClient};
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
            if let Some(keys) = page.take_results() {
                if !keys.is_empty() {
                    let deleted = keys.len() as u64;
                    self.client
                        .del::<(), _>(keys)
                        .await
                        .map_err(CacheError::CommandFailed)?;
                    count += deleted;
                }
            }
            page.next();
        }

        Ok(count)
    }
}
