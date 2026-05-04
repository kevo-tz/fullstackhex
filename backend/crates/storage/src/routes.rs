//! Storage routes.
//!
//! PUT /storage/{key}, GET /storage/{key}, DELETE /storage/{key},
//! GET /storage?prefix={prefix}, POST /storage/presign

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
    Path(key): Path<String>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, ApiError> {
    super::client::upload(&state.client, &state.config, &key, body.to_vec(), "application/octet-stream").await?;

    Ok(StatusCode::CREATED)
}

/// GET /storage/{key} — download file.
pub async fn download(
    State(state): State<StorageState>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
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
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    super::client::delete(&state.client, &state.config, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /storage — list objects with optional prefix.
pub async fn list(
    State(state): State<StorageState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let prefix = query.prefix.unwrap_or_default();
    let objects = super::client::list(&state.client, &state.config, &prefix).await?;
    Ok(Json(objects))
}

/// POST /storage/presign — generate a presigned URL.
pub async fn presign(
    State(state): State<StorageState>,
    Json(body): Json<PresignRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let method = body.method.unwrap_or_else(|| "GET".to_string());
    let expiry_secs = body.expiry_secs.unwrap_or(3600);

    let url = super::client::presigned_url(
        &state.config,
        &body.key,
        &method,
        std::time::Duration::from_secs(expiry_secs),
    )?;

    Ok(Json(PresignResponse {
        url,
        method,
        expires_in: expiry_secs,
    }))
}
