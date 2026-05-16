//! Shared error types for the FullStackHex API.
//!
//! All domain crates (auth, cache, storage) convert their errors to `ApiError`.
//! The api crate converts `ApiError` to JSON response.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Unified API error type.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    ValidationError(String),
    #[error("rate limited: {0}")]
    RateLimited(String),
    #[error("internal error: {0}")]
    InternalError(String),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl From<cache::CacheError> for ApiError {
    fn from(e: cache::CacheError) -> Self {
        match e {
            cache::CacheError::NotConfigured => {
                ApiError::ServiceUnavailable("Redis not configured".to_string())
            }
            cache::CacheError::ConnectionFailed(msg) => {
                ApiError::ServiceUnavailable(format!("Redis connection failed: {msg}"))
            }
            cache::CacheError::CommandFailed(e) => {
                ApiError::InternalError(format!("Redis error: {e}"))
            }
            cache::CacheError::SerializationFailed(msg) => ApiError::InternalError(msg),
            cache::CacheError::SessionNotFound => {
                ApiError::Unauthorized("Session not found".to_string())
            }
            cache::CacheError::RateLimitExceeded => {
                ApiError::RateLimited("Rate limit exceeded".to_string())
            }
            cache::CacheError::BackoffBlocked {
                remaining_secs,
                count,
                label,
            } => ApiError::RateLimited(format!(
                "Too many attempts ({} failures). Try again in {} seconds ({} cooldown).",
                count, remaining_secs, label
            )),
        }
    }
}

impl From<db::DbError> for ApiError {
    fn from(e: db::DbError) -> Self {
        match e {
            db::DbError::NotConfigured => {
                ApiError::ServiceUnavailable("Database not configured".to_string())
            }
            db::DbError::PoolTimeout(_) => {
                ApiError::ServiceUnavailable("Database pool timeout".to_string())
            }
            db::DbError::QueryFailed(e) => ApiError::InternalError(format!("Database error: {e}")),
            db::DbError::MigrationFailed(e) => {
                ApiError::InternalError(format!("Migration error: {e}"))
            }
        }
    }
}

#[cfg(feature = "api")]
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            ApiError::ValidationError(msg) => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone())
            }
            ApiError::RateLimited(msg) => {
                (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", msg.clone())
            }
            ApiError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                msg.clone(),
            ),
            ApiError::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                msg.clone(),
            ),
        };

        let body = json!({
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn unauthorized_returns_401() {
        let err = ApiError::Unauthorized("test".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"]["code"], "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn validation_error_returns_400() {
        let err = ApiError::ValidationError("bad input".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn rate_limited_returns_429() {
        let err = ApiError::RateLimited("slow down".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn service_unavailable_returns_503() {
        let err = ApiError::ServiceUnavailable("down".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn not_found_returns_404() {
        let err = ApiError::NotFound("missing".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn forbidden_returns_403() {
        let err = ApiError::Forbidden("denied".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn internal_error_returns_500() {
        let err = ApiError::InternalError("oops".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn cache_error_not_configured_converts_to_service_unavailable() {
        let err = ApiError::from(cache::CacheError::NotConfigured);
        assert!(matches!(err, ApiError::ServiceUnavailable(_)));
    }

    #[test]
    fn cache_error_connection_failed_converts_to_service_unavailable() {
        let err = ApiError::from(cache::CacheError::ConnectionFailed("timeout".into()));
        assert!(matches!(err, ApiError::ServiceUnavailable(_)));
    }

    #[test]
    fn cache_error_rate_limit_converts_to_429() {
        let err = ApiError::from(cache::CacheError::RateLimitExceeded);
        assert!(matches!(err, ApiError::RateLimited(_)));
    }

    #[test]
    fn cache_error_backoff_blocked_converts_to_rate_limited() {
        let err = ApiError::from(cache::CacheError::BackoffBlocked {
            remaining_secs: 42,
            count: 5,
            label: "60s".into(),
        });
        assert!(matches!(err, ApiError::RateLimited(_)));
    }

    #[test]
    fn cache_error_session_not_found_converts_to_unauthorized() {
        let err = ApiError::from(cache::CacheError::SessionNotFound);
        assert!(matches!(err, ApiError::Unauthorized(_)));
    }

    #[test]
    fn db_error_not_configured_converts_to_service_unavailable() {
        let err = ApiError::from(db::DbError::NotConfigured);
        assert!(matches!(err, ApiError::ServiceUnavailable(_)));
    }

    #[test]
    fn db_error_pool_timeout_converts_to_service_unavailable() {
        let err = ApiError::from(db::DbError::PoolTimeout(std::time::Duration::from_secs(3)));
        assert!(matches!(err, ApiError::ServiceUnavailable(_)));
    }
}
