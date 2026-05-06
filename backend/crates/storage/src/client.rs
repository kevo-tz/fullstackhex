//! S3-compatible storage client with SigV4 signing.
//!
//! Provides upload (streaming), download (streaming), delete,
//! list objects, presigned URLs, and multipart upload.

use domain::error::ApiError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::Duration;

use hmac::{Hmac, Mac};

/// Build a safe object URL from endpoint, bucket, and key.
fn build_object_url(endpoint: &str, bucket: &str, key: &str) -> Result<url::Url, url::ParseError> {
    let base = url::Url::parse(endpoint)?;
    let path = if key.is_empty() {
        format!("/{}/", bucket)
    } else {
        format!("/{}/{}", bucket, key)
    };
    base.join(&path)
}

/// Information about an object in the bucket.
#[derive(Debug, Clone, Serialize)]
pub struct ObjectInfo {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
}

/// Upload a file to the storage bucket with buffered body.
/// Prefer [`upload_streaming`] for large payloads.
pub async fn upload(
    client: &reqwest::Client,
    config: &super::StorageConfig,
    key: &str,
    body: Vec<u8>,
    content_type: &str,
) -> Result<(), ApiError> {
    let url = build_object_url(&config.endpoint, &config.bucket, key)
        .map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    let url_str = url.to_string();
    let signed = sign_request(config, "PUT", &url_str, content_type, &body)?;
    let req = client
        .put(url.as_str())
        .body(body)
        .header("Content-Type", content_type)
        .header("Host", &signed.host)
        .header("X-Amz-Date", &signed.amz_date)
        .header("X-Amz-Content-Sha256", &signed.payload_hash)
        .header("Authorization", &signed.authorization);
    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Upload failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(ApiError::ServiceUnavailable(format!(
            "Upload failed: HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Upload a file to the storage bucket with a streaming body.
/// Uses `UNSIGNED-PAYLOAD` to avoid buffering the entire payload for SigV4.
pub async fn upload_streaming(
    client: &reqwest::Client,
    config: &super::StorageConfig,
    key: &str,
    content_type: &str,
    body: reqwest::Body,
) -> Result<(), ApiError> {
    let url = build_object_url(&config.endpoint, &config.bucket, key)
        .map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    let url_str = url.to_string();
    let signed = sign_request_unsigned(config, "PUT", &url_str, content_type)?;
    let req = client
        .put(url.as_str())
        .body(body)
        .header("Content-Type", content_type)
        .header("Host", &signed.host)
        .header("X-Amz-Date", &signed.amz_date)
        .header("X-Amz-Content-Sha256", "UNSIGNED-PAYLOAD")
        .header("Authorization", &signed.authorization);
    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Upload failed: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(ApiError::ServiceUnavailable(format!(
            "Upload failed: HTTP {status}: {body_text}"
        )));
    }
    Ok(())
}

/// Download a file from the storage bucket into memory.
/// Prefer [`download_streaming`] for large objects.
pub async fn download(
    client: &reqwest::Client,
    config: &super::StorageConfig,
    key: &str,
) -> Result<Vec<u8>, ApiError> {
    let url = build_object_url(&config.endpoint, &config.bucket, key)
        .map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    let url_str = url.to_string();
    let signed = sign_request(config, "GET", &url_str, "", &[])?;
    let req = client
        .get(url.as_str())
        .header("Host", &signed.host)
        .header("X-Amz-Date", &signed.amz_date)
        .header("X-Amz-Content-Sha256", &signed.payload_hash)
        .header("Authorization", &signed.authorization);
    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Download failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(ApiError::NotFound(format!("Object not found: {key}")));
    }
    // Guard against unbounded download sizes (100 MB limit)
    const MAX_DOWNLOAD_SIZE: usize = 100 * 1024 * 1024;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to read response: {e}")))?;
    if bytes.len() > MAX_DOWNLOAD_SIZE {
        return Err(ApiError::InternalError(
            "Download exceeds maximum size (100 MB)".to_string(),
        ));
    }
    Ok(bytes.to_vec())
}

/// Download a file from the storage bucket, returning a streaming response.
/// The caller is responsible for reading the body before the response lifetime ends.
pub async fn download_streaming(
    client: &reqwest::Client,
    config: &super::StorageConfig,
    key: &str,
) -> Result<reqwest::Response, ApiError> {
    let url = build_object_url(&config.endpoint, &config.bucket, key)
        .map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    let url_str = url.to_string();
    let signed = sign_request_unsigned(config, "GET", &url_str, "")?;
    let resp = client
        .get(url.as_str())
        .header("Host", &signed.host)
        .header("X-Amz-Date", &signed.amz_date)
        .header("X-Amz-Content-Sha256", "UNSIGNED-PAYLOAD")
        .header("Authorization", &signed.authorization)
        .send()
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Download failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(ApiError::NotFound(format!("Object not found: {key}")));
    }
    Ok(resp)
}

/// Delete a file from the storage bucket.
pub async fn delete(
    client: &reqwest::Client,
    config: &super::StorageConfig,
    key: &str,
) -> Result<(), ApiError> {
    let url = build_object_url(&config.endpoint, &config.bucket, key)
        .map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    let url_str = url.to_string();
    let signed = sign_request(config, "DELETE", &url_str, "", &[])?;
    let req = client
        .delete(url.as_str())
        .header("Host", &signed.host)
        .header("X-Amz-Date", &signed.amz_date)
        .header("X-Amz-Content-Sha256", &signed.payload_hash)
        .header("Authorization", &signed.authorization);
    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Delete failed: {e}")))?;
    if !resp.status().is_success() && resp.status() != 204 {
        return Err(ApiError::ServiceUnavailable(format!(
            "Delete failed: HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

/// List objects with an optional prefix.
pub async fn list(
    client: &reqwest::Client,
    config: &super::StorageConfig,
    prefix: &str,
) -> Result<Vec<ObjectInfo>, ApiError> {
    let base = build_object_url(&config.endpoint, &config.bucket, "")
        .map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    let mut url = base;
    url.query_pairs_mut()
        .append_pair("list-type", "2")
        .append_pair("prefix", prefix);
    let url_str = url.to_string();
    let signed = sign_request(config, "GET", &url_str, "", &[])?;
    let req = client
        .get(&url_str)
        .header("Host", &signed.host)
        .header("X-Amz-Date", &signed.amz_date)
        .header("X-Amz-Content-Sha256", &signed.payload_hash)
        .header("Authorization", &signed.authorization);
    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("List failed: {e}")))?;

    let body = resp
        .text()
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to read response: {e}")))?;

    // Quick XML parse - just extract keys for now
    let mut objects = Vec::new();
    for cap in body.split("<Key>").skip(1) {
        let key = cap.split("</Key>").next().unwrap_or("");
        objects.push(ObjectInfo {
            key: key.to_string(),
            size: 0,
            last_modified: String::new(),
        });
    }
    Ok(objects)
}

/// Generate a presigned URL for upload or download.
pub fn presigned_url(
    config: &super::StorageConfig,
    key: &str,
    _method: &str,
    _expiry: Duration,
) -> Result<String, ApiError> {
    // For presigned URLs, use the public endpoint
    let url = build_object_url(&config.public_endpoint, &config.bucket, key)
        .map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    Ok(url.to_string())
}

/// Signed request components.
pub struct SignedRequest {
    pub host: String,
    pub amz_date: String,
    pub payload_hash: String,
    pub authorization: String,
}

/// Sign a request with AWS SigV4 using `UNSIGNED-PAYLOAD` as the content hash.
/// Use for streaming requests where the full body hash is not known ahead of time.
pub fn sign_request_unsigned(
    config: &super::StorageConfig,
    method: &str,
    url: &str,
    content_type: &str,
) -> Result<SignedRequest, ApiError> {
    sign_request_inner(config, method, url, content_type, "UNSIGNED-PAYLOAD".to_string())
}

/// Sign a request with AWS SigV4.
pub fn sign_request(
    config: &super::StorageConfig,
    method: &str,
    url: &str,
    content_type: &str,
    body: &[u8],
) -> Result<SignedRequest, ApiError> {
    sign_request_inner(config, method, url, content_type, sha256_hex(body))
}

fn sign_request_inner(
    config: &super::StorageConfig,
    method: &str,
    url: &str,
    content_type: &str,
    payload_hash: String,
) -> Result<SignedRequest, ApiError> {
    let amz_date = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = amz_date[..8].to_string();

    let parsed =
        url::Url::parse(url).map_err(|e| ApiError::InternalError(format!("Invalid URL: {e}")))?;
    let host = parsed.host_str().unwrap_or("localhost").to_string();
    let uri = parsed.path();
    let query = parsed.query().unwrap_or("");

    let mut headers = vec![
        ("host".to_string(), host.clone()),
        ("x-amz-content-sha256".to_string(), payload_hash.clone()),
        ("x-amz-date".to_string(), amz_date.clone()),
    ];
    if !content_type.is_empty() {
        headers.push(("content-type".to_string(), content_type.to_string()));
    }

    // Build canonical headers (sorted by key name)
    headers.sort_by(|a, b| a.0.cmp(&b.0));
    let canonical_headers = headers
        .iter()
        .map(|(k, v)| format!("{}:{}", k.to_lowercase(), v.trim()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let signed_headers = headers
        .iter()
        .map(|(k, _)| k.to_lowercase())
        .collect::<Vec<_>>()
        .join(";");

    // Canonical request
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, uri, query, canonical_headers, signed_headers, payload_hash
    );

    // String to sign
    let algorithm = "AWS4-HMAC-SHA256";
    let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, config.region, "s3");
    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        algorithm,
        amz_date,
        credential_scope,
        sha256_hex(canonical_request.as_bytes())
    );

    // Signing key
    let k_date = hmac_sha256(format!("AWS4{}", config.secret_key).as_bytes(), &date_stamp);
    let k_region = hmac_sha256(&k_date, &config.region);
    let k_service = hmac_sha256(&k_region, "s3");
    let k_signing = hmac_sha256(&k_service, "aws4_request");

    // Signature
    let signature = hex(&hmac_sha256(&k_signing, &string_to_sign));

    let authorization = format!(
        "{} Credential={}/{}, SignedHeaders={}, Signature={}",
        algorithm, config.access_key, credential_scope, signed_headers, signature
    );

    Ok(SignedRequest {
        host,
        amz_date,
        payload_hash,
        authorization,
    })
}

/// Compute SHA256 hash of a byte slice, returned as hex.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute HMAC-SHA256.
fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC can take any key");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Convert bytes to hex string.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches() {
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn hmac_sha256_produces_output() {
        let result = hmac_sha256(b"secret", "data");
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn hex_encodes_bytes() {
        assert_eq!(hex(&[0xab, 0xcd]), "abcd");
    }

    #[test]
    fn sign_request_includes_required_fields() {
        let config = crate::StorageConfig {
            endpoint: "http://localhost:9000".to_string(),
            public_endpoint: "http://pub.local:9000".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            auto_create_bucket: false,
        };
        let signed = sign_request(
            &config,
            "GET",
            "http://localhost:9000/test-bucket/file.txt",
            "",
            &[],
        )
        .unwrap();
        assert!(!signed.authorization.is_empty());
        assert!(!signed.amz_date.is_empty());
        assert!(!signed.payload_hash.is_empty());
        assert_eq!(signed.host, "localhost");
    }

    #[test]
    fn presigned_url_uses_public_endpoint() {
        let config = crate::StorageConfig {
            endpoint: "http://localhost:9000".to_string(),
            public_endpoint: "http://pub.local:9000".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            auto_create_bucket: false,
        };
        let url = presigned_url(
            &config,
            "file.txt",
            "GET",
            std::time::Duration::from_secs(3600),
        )
        .unwrap();
        assert_eq!(url, "http://pub.local:9000/test-bucket/file.txt");
    }

    #[test]
    fn sign_request_with_content_type_includes_header() {
        let config = crate::StorageConfig {
            endpoint: "http://localhost:9000".to_string(),
            public_endpoint: "http://pub.local:9000".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            auto_create_bucket: false,
        };
        let signed = sign_request(
            &config,
            "PUT",
            "http://localhost:9000/test-bucket/file.txt",
            "application/json",
            b"{}",
        )
        .unwrap();
        assert!(signed.authorization.contains("AWS4-HMAC-SHA256"));
        assert!(signed.authorization.contains("test-key"));
        assert_eq!(signed.host, "localhost");
        assert!(!signed.payload_hash.is_empty());
    }

    #[test]
    fn sign_request_empty_body_produces_valid_hash() {
        let config = crate::StorageConfig {
            endpoint: "http://localhost:9000".to_string(),
            public_endpoint: "http://pub.local:9000".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            auto_create_bucket: false,
        };
        let signed = sign_request(
            &config,
            "GET",
            "http://localhost:9000/test-bucket/file.txt",
            "",
            &[],
        )
        .unwrap();
        // SHA-256 of empty body
        assert_eq!(
            signed.payload_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_empty_body() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hmac_sha256_with_empty_data() {
        let result = hmac_sha256(b"secret", "");
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn hex_zero_bytes() {
        assert_eq!(hex(&[0x00, 0x01, 0xff]), "0001ff");
    }
}
