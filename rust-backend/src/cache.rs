use redis::{Client, Commands, RedisResult};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct CacheClient {
    client: Arc<Mutex<redis::Connection>>,
}

impl CacheClient {
    /// Initialize Redis connection
    pub fn new(redis_url: &str) -> anyhow::Result<Self> {
        let client = Client::open(redis_url)?;
        let connection = client.get_connection()?;
        Ok(CacheClient {
            client: Arc::new(Mutex::new(connection)),
        })
    }

    /// Get value from cache
    pub fn get(&self, key: &str) -> RedisResult<Option<String>> {
        let mut conn = self.client.lock().unwrap();
        conn.get(key)
    }

    /// Set value in cache
    pub fn set(&self, key: &str, value: &str) -> RedisResult<()> {
        let mut conn = self.client.lock().unwrap();
        conn.set(key, value)
    }

    /// Set value in cache with expiration in seconds
    pub fn set_ex(&self, key: &str, value: &str, ttl_seconds: u64) -> RedisResult<()> {
        let mut conn = self.client.lock().unwrap();
        conn.set_ex(key, value, ttl_seconds)
    }

    /// Delete value from cache
    pub fn delete(&self, key: &str) -> RedisResult<()> {
        let mut conn = self.client.lock().unwrap();
        conn.del(key)
    }

    /// Check if key exists in cache
    pub fn exists(&self, key: &str) -> RedisResult<bool> {
        let mut conn = self.client.lock().unwrap();
        conn.exists(key)
    }
}
