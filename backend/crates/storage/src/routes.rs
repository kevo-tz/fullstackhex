//! Storage routes.
//!
//! PUT /storage/{key}, GET /storage/{key}, DELETE /storage/{key},
//! GET /storage?prefix={prefix}, POST /storage/presign
//!
//! All routes require authentication. Object keys are prefixed with
//! `users/{user_id}/` to enforce per-user isolation.

use auth::middleware::AuthUser;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use domain::error::ApiError;
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

/// PUT /storage/{key} — upload file.
pub async fn upload(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path(key): Path<String>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    super::client::upload(&state.client, &state.config, &key, body.to_vec(), "application/octet-stream").await?;

    Ok(StatusCode::CREATED)
}

/// GET /storage/{key} — download file.
pub async fn download(
    State(state): State<StorageState>,
    auth_user: AuthUser,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let key = user_key(&auth_user.user_id, &key);
    let data = super::client::download(&state.client, &state.config, &key).await?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        data,
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
    let prefix = query.prefix.map(|p| user_key(&auth_user.user_id, &p))
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

/// Prefix a storage key with the user's namespace.
fn user_key(user_id: &str, key: &str) -> String {
    format!("users/{}/{}", user_id, key)
}
