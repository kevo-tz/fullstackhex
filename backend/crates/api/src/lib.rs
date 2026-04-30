use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use std::env;

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/health/db", get(health_db))
        .route("/health/python", get(health_python))
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn health_db() -> Json<Value> {
    // TODO: wire up real sqlx pool once DATABASE_URL is configured
    let database_url = env::var("DATABASE_URL").unwrap_or_default();
    if database_url.is_empty() {
        Json(json!({ "status": "error", "error": "DATABASE_URL not configured" }))
    } else {
        Json(json!({ "status": "ok" }))
    }
}

async fn health_python() -> Json<Value> {
    // TODO: wire up real Unix socket check once sidecar is running
    let socket_path = env::var("PYTHON_SIDECAR_SOCKET")
        .unwrap_or_else(|_| "/tmp/python-sidecar.sock".to_string());

    if std::path::Path::new(&socket_path).exists() {
        Json(json!({ "status": "ok", "version": "unknown" }))
    } else {
        Json(json!({ "status": "unavailable", "error": "socket not found" }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_200() {
        let app = router();
        let response = app
            .oneshot(Request::builder().uri("/health").body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_response_has_status_ok() {
        let app = router();
        let response = app
            .oneshot(Request::builder().uri("/health").body(axum::body::Body::empty()).unwrap())
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
        let app = router();
        let response = app
            .oneshot(Request::builder().uri("/health/db").body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_python_returns_200() {
        let app = router();
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
}
