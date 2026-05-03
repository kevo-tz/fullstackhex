use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::{Json, Router, extract::State, http::Request, middleware, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use python_sidecar::PythonSidecar;
#[cfg(test)]
use serde_json::Value;
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

pub mod metrics;

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
    pub prometheus_handle: PrometheusHandle,
    /// Abort handle for the background DB pool gauge task.
    /// Only present when the database is connected.
    pub gauge_task: Option<tokio::task::AbortHandle>,
}

/// Build the router with default state (from environment).
/// Returns the router and the shared state so the caller can clean up
/// background tasks (e.g. abort the DB pool gauge) after shutdown.
pub async fn router(prometheus_handle: PrometheusHandle) -> (Router, Arc<AppState>) {
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

    let gauge_task = match &db {
        DbStatus::Connected(pool) => Some(metrics::spawn_pool_gauge_task(pool.clone())),
        _ => None,
    };

    let state = Arc::new(AppState {
        db,
        sidecar: PythonSidecar::from_env(),
        prometheus_handle,
        gauge_task,
    });

    (build_router(state.clone()), state)
}

/// Build the router with explicit state (for testing).
pub fn router_with_state(state: AppState) -> Router {
    build_router(Arc::new(state))
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/health/db", get(health_db))
        .route("/health/python", get(health_python))
        .route("/metrics", get(metrics_handler))
        .route("/metrics/python", get(metrics_python_proxy))
        .layer(middleware::from_fn(metrics::track_metrics))
        .with_state(state)
}

fn no_cache() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache, no-store"),
    );
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
                StatusCode::SERVICE_UNAVAILABLE,
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
            (
                StatusCode::SERVICE_UNAVAILABLE,
                no_cache(),
                Json(json!({ "status": "error", "error": error, "fix": fix })),
            )
        }
    }
}

async fn health_python(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    let trace_id = req
        .headers()
        .get("x-trace-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !trace_id.is_empty() {
        tracing::info!(%trace_id, "health check via sidecar with propagated trace_id");
    }

    let result = if trace_id.is_empty() {
        state.sidecar.health().await
    } else {
        state.sidecar.get_with_trace_id("/health", trace_id).await
    };

    match result {
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
                python_sidecar::SidecarError::InvalidInput(msg) => (
                    format!("invalid input: {msg}"),
                    "The request contains invalid characters.".to_string(),
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
            (
                StatusCode::SERVICE_UNAVAILABLE,
                no_cache(),
                Json(json!({ "status": "unavailable", "error": error_msg, "fix": fix_msg })),
            )
        }
    }
}

async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let body = metrics::render_metrics(&state.prometheus_handle);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
}

async fn metrics_python_proxy(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.sidecar.get_raw("/metrics").await {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            body,
        ),
        Err(python_sidecar::SidecarError::HttpError { status, body }) => {
            let code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            tracing::warn!(status = %status, "Python sidecar returned HTTP error for /metrics");
            (
                code,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                format!("# Python metrics error: {body}").into_bytes(),
            )
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to proxy Python metrics");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                format!("# Python metrics unavailable: {e}").into_bytes(),
            )
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
            prometheus_handle: metrics::init_metrics_recorder(),
            gauge_task: None,
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
    async fn health_db_returns_503_when_not_configured() {
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
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
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
    async fn health_python_returns_503_when_no_socket() {
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
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
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

    #[tokio::test]
    async fn health_python_with_trace_id_header_returns_unavailable() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/python")
                    .header("x-trace-id", "test-trace-abc-123")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "unavailable");
        assert!(v["error"].is_string());
    }

    #[tokio::test]
    async fn health_python_with_empty_trace_id_returns_unavailable() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/python")
                    .header("x-trace-id", "")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "unavailable");
        assert!(v["error"].is_string());
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_prometheus_text() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain; charset=utf-8"
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(
            body.contains("# HELP") || body.contains("http_requests_total"),
            "metrics body should contain prometheus content: {}",
            body
        );
    }

    #[tokio::test]
    async fn middleware_increments_request_counter() {
        let app = router_with_state(test_state());
        // Make a request to /health
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Now check /metrics for the counter
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(
            body.contains("http_requests_total"),
            "metrics should contain http_requests_total: {}",
            body
        );
    }

    #[tokio::test]
    async fn middleware_records_request_histogram() {
        let app = router_with_state(test_state());
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(
            body.contains("http_request_duration_seconds_bucket"),
            "metrics should contain http_request_duration_seconds_bucket: {}",
            body
        );
    }

    #[tokio::test]
    async fn metrics_python_proxy_unavailable_when_no_socket() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics/python")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn normalize_route_bounds_known_paths() {
        assert_eq!(metrics::normalize_route("/health"), "/health");
        assert_eq!(metrics::normalize_route("/health/db"), "/health/db");
        assert_eq!(metrics::normalize_route("/health/python"), "/health/python");
        assert_eq!(metrics::normalize_route("/metrics"), "/metrics");
        assert_eq!(
            metrics::normalize_route("/metrics/python"),
            "/metrics/python"
        );
    }

    #[tokio::test]
    async fn normalize_route_collapses_unknown_paths() {
        assert_eq!(metrics::normalize_route("/api/users/123"), "unknown");
        assert_eq!(metrics::normalize_route("/admin"), "unknown");
    }
}
