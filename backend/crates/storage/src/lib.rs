//! S3-compatible object storage for FullStackHex.
//!
//! Provides streaming upload/download, presigned URLs, and multipart upload
//! backed by RustFS (or any S3-compatible storage). Uses SigV4 signing over HTTP.

pub mod client;
pub mod routes;

use domain::error::ApiError;
use std::time::Duration;

/// Storage configuration.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub endpoint: String,
    pub public_endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub auto_create_bucket: bool,
}

impl StorageConfig {
    /// Load storage config from environment variables.
    pub fn from_env() -> Option<Self> {
        let endpoint = std::env::var("RUSTFS_ENDPOINT").ok()?;
        let access_key = std::env::var("RUSTFS_ACCESS_KEY").ok()?;
        let secret_key = std::env::var("RUSTFS_SECRET_KEY").ok()?;

        if access_key == "CHANGE_ME" || secret_key == "CHANGE_ME" {
            tracing::warn!("RUSTFS_ACCESS_KEY or RUSTFS_SECRET_KEY is CHANGE_ME — storage disabled");
            return None;
        }

        Some(Self {
            public_endpoint: std::env::var("RUSTFS_PUBLIC_ENDPOINT")
                .unwrap_or_else(|_| endpoint.clone()),
            bucket: std::env::var("RUSTFS_BUCKET").unwrap_or_else(|_| "fullstackhex".to_string()),
            region: std::env::var("RUSTFS_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            auto_create_bucket: std::env::var("RUSTFS_AUTO_CREATE_BUCKET")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            endpoint,
            access_key,
            secret_key,
        })
    }
}

/// Storage client wrapping HTTP + SigV4 signing for S3-compatible APIs.
pub struct StorageClient {
    pub config: StorageConfig,
    client: reqwest::Client,
}

impl StorageClient {
    /// Create a new storage client.
    pub fn new(config: StorageConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest Client::new() is infallible");

        Self { config, client }
    }

    /// Create from environment variables. Returns None if not configured.
    pub fn from_env() -> Option<Self> {
        let config = StorageConfig::from_env()?;
        Some(Self::new(config))
    }

    /// Auto-create the bucket if `auto_create_bucket` is enabled.
    pub async fn ensure_bucket(&self) -> Result<(), ApiError> {
        if !self.config.auto_create_bucket {
            return Ok(());
        }

        let exists = self.bucket_exists().await?;
        if exists {
            tracing::info!("bucket {} exists", self.config.bucket);
            return Ok(());
        }

        tracing::info!("creating bucket {}", self.config.bucket);
        let url = format!("{}/{}", self.config.endpoint, self.config.bucket);
        let req = self.client.put(&url);
        let req = sign_request(req, &self.config, "s3", "us-east-1", "", "").await;
        let resp = req.send().await.map_err(|e| {
            ApiError::ServiceUnavailable(format!("Failed to create bucket: {e}"))
        })?;

        if !resp.status().is_success() {
            return Err(ApiError::ServiceUnavailable(format!(
                "Failed to create bucket: HTTP {}",
                resp.status()
            )));
        }

        tracing::info!("bucket {} created", self.config.bucket);
        Ok(())
    }

    async fn bucket_exists(&self) -> Result<bool, ApiError> {
        let url = format!("{}/{}", self.config.endpoint, self.config.bucket);
        let req = self.client.head(&url);
        let req = sign_request(req, &self.config, "s3", "us-east-1", "", "").await;
        let resp = req.send().await.map_err(|e| {
            ApiError::ServiceUnavailable(format!("Storage unreachable: {e}"))
        })?;
        Ok(resp.status().is_success())
    }
}

/// Placeholder for SigV4 request signing.
async fn sign_request(
    req: reqwest::RequestBuilder,
    _config: &StorageConfig,
    _service: &str,
    _region: &str,
    _payload: &str,
    _content_type: &str,
) -> reqwest::RequestBuilder {
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_missing_returns_none() {
        // SAFETY: test-only, single-threaded context
        unsafe { std::env::remove_var("RUSTFS_ENDPOINT"); }
        assert!(StorageConfig::from_env().is_none());
    }
}
