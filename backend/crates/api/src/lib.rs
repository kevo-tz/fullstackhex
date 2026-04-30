use axum::{Json, Router, extract::State, routing::get};
use python_sidecar::PythonSidecar;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

pub struct AppState {
    pub db_pool: Option<PgPool>,
    pub sidecar: PythonSidecar,
}

/// Build the router with default state (from environment).
pub async fn router() -> Router {
    let pool = match std::env::var("DATABASE_URL") {
        Ok(url) => PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(2))
            .connect(&url)
            .await
            .ok(),
        Err(_) => None,
    };

    let state = AppState {
        db_pool: pool,
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

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn health_db(State(state): State<Arc<AppState>>) -> Json<Value> {
    match db::health_check(state.db_pool.as_ref()).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "status": "error", "error": e.to_string() })),
    }
}

async fn health_python(State(state): State<Arc<AppState>>) -> Json<Value> {
    match state.sidecar.health().await {
        Ok(v) => Json(json!({
            "status": v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown"),
            "service": v.get("service").and_then(|s| s.as_str()).unwrap_or("unknown"),
            "version": v.get("version").and_then(|s| s.as_str()).unwrap_or("unknown"),
        })),
        Err(e) => {
            let error_msg = match &e {
                python_sidecar::SidecarError::SocketNotFound(_) => "socket not found".to_string(),
                python_sidecar::SidecarError::ConnectionFailed(msg) => {
                    format!("connection failed: {msg}")
                }
                python_sidecar::SidecarError::Timeout(d) => {
                    format!("request timed out after {d:?}")
                }
                python_sidecar::SidecarError::InvalidResponse(msg) => {
                    format!("invalid response: {msg}")
                }
                python_sidecar::SidecarError::HttpError { status, body } => {
                    format!("HTTP {status}: {body}")
                }
            };
            Json(json!({ "status": "unavailable", "error": error_msg }))
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
            db_pool: None,
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
