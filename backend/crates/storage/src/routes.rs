//! Storage routes.
//!
//! PUT /storage/{key}, GET /storage/{key}, DELETE /storage/{key},
//! GET /storage?prefix={prefix}, POST /storage/presign
//!
//! All routes require authentication. Object keys are prefixed with
//! `users/{user_id}/` to enforce per-user isolation.

use auth::middleware::AuthUser;
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use domain::error::ApiError;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

/// Shared state for storage routes.
#[derive(Clone)]
pub struct StorageState {
    pub client: reqwest::Client,
    pub config: super::StorageConfig,
}

/// Presigned URL request body.
#[derive(Debug, Deserialize)]
pub struct PresignRequest {
    pub key: String,
    pub method: Option<String>,
    pub expiry_secs: Option<u64>,
}

/// Presigned URL response.
#[derive(Debug, Serialize)]
pub struct PresignResponse {
    pub url: String,
    pub method: String,
    pub expires_in: u64,
}

/// Query params for listing.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub prefix: Option<String>,
}

/// PUT /storage/{key} — upload a file with streaming body.
pub async fn upload(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path(key): Path<String>,
    body: Body,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    let stream = body.into_data_stream();
    let reqwest_body = reqwest::Body::wrap_stream(stream);
    super::client::upload_streaming(
        &state.client,
        &state.config,
        &key,
        "application/octet-stream",
        reqwest_body,
    )
    .await?;

    Ok(StatusCode::CREATED)
}

/// GET /storage/{key} — download a file with streaming body.
pub async fn download(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    let resp = super::client::download_streaming(&state.client, &state.config, &key).await?;

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let stream = resp.bytes_stream().map(|r| {
        r.map_err(|e| {
            axum::Error::new(std::io::Error::new(std::io::ErrorKind::Other, e))
        })
    });
    let body = Body::from_stream(stream);

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, content_type)],
        body,
    ))
}

/// DELETE /storage/{key} — delete file.
pub async fn delete(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    super::client::delete(&state.client, &state.config, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /storage — list objects with optional prefix.
pub async fn list(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let prefix = query
        .prefix
        .map(|p| user_key(&auth_user.user_id, &p))
        .unwrap_or_else(|| format!("users/{}/", auth_user.user_id));
    let objects = super::client::list(&state.client, &state.config, &prefix).await?;
    Ok(Json(objects))
}

/// POST /storage/presign — generate a presigned URL.
pub async fn presign(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Json(body): Json<PresignRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let method = body.method.unwrap_or_else(|| "GET".to_string());
    let expiry_secs = body.expiry_secs.unwrap_or(3600);
    let key = user_key(&auth_user.user_id, &body.key);

    let url = super::client::presigned_url(
        &state.config,
        &key,
        &method,
        std::time::Duration::from_secs(expiry_secs),
    )?;

    Ok(Json(PresignResponse {
        url,
        method,
        expires_in: expiry_secs,
    }))
}

// ── Multipart upload handlers ──────────────────────────────────────────

/// Request body for initiating a multipart upload.
#[derive(Debug, Deserialize)]
pub struct MultipartInitRequest {
    pub key: String,
    pub content_type: Option<String>,
}

/// Request body for completing a multipart upload.
#[derive(Debug, Deserialize)]
pub struct MultipartCompleteRequest {
    pub parts: Vec<super::client::PartInfo>,
}

/// POST /storage/multipart/init — initiate a multipart upload.
pub async fn init_multipart(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Json(body): Json<MultipartInitRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &body.key);
    let content_type = body.content_type.as_deref().unwrap_or("application/octet-stream");
    let upload = super::client::create_multipart_upload(
        &state.client,
        &state.config,
        &key,
        content_type,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(upload)))
}

/// PUT /storage/multipart/{key}/{upload_id}/part/{part_number} — upload a part.
pub async fn upload_part(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path((key, upload_id, part_number)): Path<(String, String, u32)>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    let part = super::client::upload_part(
        &state.client,
        &state.config,
        &key,
        &upload_id,
        part_number,
        body.to_vec(),
    )
    .await?;
    Ok((StatusCode::OK, Json(part)))
}

/// POST /storage/multipart/{key}/{upload_id}/complete — complete a multipart upload.
pub async fn complete_multipart(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path((key, upload_id)): Path<(String, String)>,
    Json(body): Json<MultipartCompleteRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    super::client::complete_multipart_upload(
        &state.client,
        &state.config,
        &key,
        &upload_id,
        &body.parts,
    )
    .await?;
    Ok(StatusCode::OK)
}

/// DELETE /storage/multipart/{key}/{upload_id} — abort a multipart upload.
pub async fn abort_multipart(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path((key, upload_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    super::client::abort_multipart_upload(
        &state.client,
        &state.config,
        &key,
        &upload_id,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Prefix a storage key with the user's namespace.
fn user_key(user_id: &str, key: &str) -> String {
    format!("users/{}/{}", user_id, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_key_prefixes_with_user_id() {
        assert_eq!(user_key("user-123", "file.txt"), "users/user-123/file.txt");
    }

    #[test]
    fn user_key_handles_nested_paths() {
        assert_eq!(
            user_key("user-123", "a/b/c.png"),
            "users/user-123/a/b/c.png"
        );
    }

    #[tokio::test]
    async fn presign_handler_returns_url() {
        let state = StorageState {
            client: reqwest::Client::new(),
            config: crate::StorageConfig {
                endpoint: "http://localhost:9000".to_string(),
                public_endpoint: "http://pub.local:9000".to_string(),
                access_key: "test-key".to_string(),
                secret_key: "test-secret".to_string(),
                bucket: "test-bucket".to_string(),
                region: "us-east-1".to_string(),
                auto_create_bucket: false,
            },
        };

        let auth_user = auth::middleware::AuthUser {
            user_id: "user-123".to_string(),
            email: "test@example.com".to_string(),
            name: None,
            provider: "local".to_string(),
            jti: "test-jti-1".to_string(),
            session_id: None,
        };

        let body = PresignRequest {
            key: "file.txt".to_string(),
            method: Some("GET".to_string()),
            expiry_secs: Some(3600),
        };

        let response = presign(State(state), auth_user, Json(body)).await;
        assert!(response.is_ok());

        let resp = response.unwrap().into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v["url"],
            "http://pub.local:9000/test-bucket/users/user-123/file.txt"
        );
        assert_eq!(v["method"], "GET");
        assert_eq!(v["expires_in"], 3600);
    }

    #[tokio::test]
    async fn presign_handler_uses_defaults() {
        let state = StorageState {
            client: reqwest::Client::new(),
            config: crate::StorageConfig {
                endpoint: "http://localhost:9000".to_string(),
                public_endpoint: "http://pub.local:9000".to_string(),
                access_key: "test-key".to_string(),
                secret_key: "test-secret".to_string(),
                bucket: "test-bucket".to_string(),
                region: "us-east-1".to_string(),
                auto_create_bucket: false,
            },
        };

        let auth_user = auth::middleware::AuthUser {
            user_id: "user-456".to_string(),
            email: "test@example.com".to_string(),
            name: None,
            provider: "local".to_string(),
            jti: "test-jti-2".to_string(),
            session_id: None,
        };

        let body = PresignRequest {
            key: "file.txt".to_string(),
            method: None,
            expiry_secs: None,
        };

        let response = presign(State(state), auth_user, Json(body)).await;
        assert!(response.is_ok());

        let resp = response.unwrap().into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["method"], "GET");
        assert_eq!(v["expires_in"], 3600);
    }
}
