use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::{Json, Router, extract::State, routing::get};
use python_sidecar::PythonSidecar;
use serde_json::json;
#[cfg(test)]
use serde_json::Value;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

/// Status of the database connection pool.
pub enum DbStatus {
    /// No DATABASE_URL configured — database is intentionally absent.
    NotConfigured,
    /// Pool created successfully.
    Connected(PgPool),
    /// DATABASE_URL was set but connecting failed.
    ConnectionFailed(String),
}

pub struct AppState {
    pub db: DbStatus,
    pub sidecar: PythonSidecar,
}

/// Build the router with default state (from environment).
pub async fn router() -> Router {
    let db = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            match PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(2))
                .connect(&url)
                .await
            {
                Ok(pool) => DbStatus::Connected(pool),
                Err(e) => DbStatus::ConnectionFailed(format!("connection failed: {e}")),
            }
        }
        Err(_) => DbStatus::NotConfigured,
    };

    let state = AppState {
        db,
        sidecar: PythonSidecar::from_env(),
    };

    router_with_state(state)
}

/// Build the router with explicit state (for testing).
pub fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/health/db", get(health_db))
        .route("/health/python", get(health_python))
        .with_state(Arc::new(state))
}

fn no_cache() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-cache, no-store"));
    headers
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        no_cache(),
        Json(json!({
            "status": "ok",
            "service": "api",
            "version": env!("CARGO_PKG_VERSION")
        })),
    )
}

async fn health_db(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pool = match &state.db {
        DbStatus::Connected(pool) => Some(pool),
        DbStatus::NotConfigured => None,
        DbStatus::ConnectionFailed(msg) => {
            return (
                StatusCode::OK,
                no_cache(),
                Json(json!({
                    "status": "error",
                    "error": msg,
                    "fix": "Check that PostgreSQL is running and DATABASE_URL is correct in .env. Then restart the backend."
                })),
            );
        }
    };

    match db::health_check(pool).await {
        Ok(()) => (StatusCode::OK, no_cache(), Json(json!({ "status": "ok" }))),
        Err(e) => {
            let (error, fix) = match &e {
                db::DbError::NotConfigured => (
                    "database not configured",
                    "Set DATABASE_URL in .env and restart the backend.",
                ),
                db::DbError::PoolTimeout(_) => (
                    "database pool timeout",
                    "The database pool is exhausted. Check PostgreSQL connection and increase DB_MAX_CONNECTIONS if needed.",
                ),
                db::DbError::QueryFailed(_) => (
                    "database query failed",
                    "Check that PostgreSQL is running and the database exists.",
                ),
            };
            (StatusCode::OK, no_cache(), Json(json!({ "status": "error", "error": error, "fix": fix })))
        }
    }
}

async fn health_python(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.sidecar.health().await {
        Ok(v) => (
            StatusCode::OK,
            no_cache(),
            Json(json!({
                "status": v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown"),
                "service": v.get("service").and_then(|s| s.as_str()).unwrap_or("unknown"),
                "version": v.get("version").and_then(|s| s.as_str()).unwrap_or("unknown"),
            })),
        ),
        Err(e) => {
            let sock_display = state.sidecar.socket_path().display();
            let (error_msg, fix_msg) = match &e {
                python_sidecar::SidecarError::SocketNotFound(_) => (
                    "socket not found".to_string(),
                    format!("Start the Python sidecar: make dev starts it automatically, or run: cd python-sidecar && uv run uvicorn app.main:app --uds {sock_display}"),
                ),
                python_sidecar::SidecarError::ConnectionFailed(msg) => (
                    format!("connection failed: {msg}"),
                    format!("Check that the Python sidecar is running. Run: cd python-sidecar && uv run uvicorn app.main:app --uds {sock_display}"),
                ),
                python_sidecar::SidecarError::Timeout(d) => (
                    format!("request timed out after {d:?}"),
                    format!("The Python sidecar is not responding. Restart it with: cd python-sidecar && uv run uvicorn app.main:app --uds {sock_display}"),
                ),
                python_sidecar::SidecarError::InvalidResponse(msg) => (
                    format!("invalid response: {msg}"),
                    "The Python sidecar returned an unexpected response. Check its logs for errors.".to_string(),
                ),
                python_sidecar::SidecarError::HttpError { status, body } => (
                    format!("HTTP {status}: {body}"),
                    "The Python sidecar returned an HTTP error. Check its logs for details.".to_string(),
                ),
            };
            (StatusCode::OK, no_cache(), Json(json!({ "status": "unavailable", "error": error_msg, "fix": fix_msg })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            db: DbStatus::NotConfigured,
            sidecar: PythonSidecar::new(
                "/tmp/__nonexistent_test_socket__.sock",
                Duration::from_secs(1),
                0,
            ),
        }
    }
    #[tokio::test]
    async fn health_returns_200() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_response_has_status_ok() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.headers().get("cache-control").unwrap(),
            "no-cache, no-store"
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["service"], "api");
        assert!(v["version"].is_string());
    }

    #[tokio::test]
    async fn health_db_returns_200() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/db")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_db_error_when_no_pool() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/db")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "error");
        assert!(v["error"].is_string());
    }

    #[tokio::test]
    async fn health_python_returns_200() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/python")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_python_unavailable_when_no_socket() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/python")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "unavailable");
        assert!(v["error"].is_string());
    }
}
