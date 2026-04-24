use anyhow::Result;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct CacheClient {
    conn: Arc<OnceCell<ConnectionManager>>,
}

impl CacheClient {
    pub fn new(redis_url: &str) -> Self {
        let conn = Arc::new(OnceCell::new());
        let url = redis_url.to_string();
        let conn_clone = conn.clone();
        
        tokio::spawn(async move {
            let client = match redis::Client::open(url.clone()) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to create Redis client: {}", e);
                    return;
                }
            };
            
            match ConnectionManager::new(client).await {
                Ok(manager) => {
                    let _ = conn_clone.set(manager);
                    tracing::info!("Redis connection manager initialized");
                }
                Err(e) => tracing::error!("Failed to create connection manager: {}", e),
            }
        });
        
        Self { conn }
    }

    async fn get_conn(&self) -> Result<ConnectionManager> {
        self.conn
            .get()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Redis connection not initialized"))
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_conn().await?;
        let result: Option<String> = conn.get(key).await?;
        Ok(result)
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.get_conn().await?;
        Ok(conn.set(key, value).await?)
    }

    pub async fn set_ex(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<()> {
        let mut conn = self.get_conn().await?;
        Ok(conn.set_ex(key, value, ttl_seconds).await?)
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let _: i32 = conn.del(key).await?;
        Ok(())
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_conn().await?;
        let result: bool = conn.exists(key).await?;
        Ok(result)
    }
}
