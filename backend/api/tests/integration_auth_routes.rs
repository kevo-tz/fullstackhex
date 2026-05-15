use api::AppState;
use api::DbStatus;
/// Integration tests verifying auth routes are absent when auth is not configured.
use api::metrics::init_metrics_recorder;
use api::router_with_state;
use axum::http::{Request, StatusCode};
use py_sidecar::PythonSidecar;
use std::time::Duration;
use tower::ServiceExt;

fn test_state_without_auth() -> AppState {
    AppState {
        db: DbStatus::NotConfigured,
        redis: None,
        auth: None,
        storage: None,
        sidecar: PythonSidecar::new(
            "/tmp/__nonexistent_test_socket__.sock",
            Duration::from_secs(1),
            0,
        ),
        prometheus_handle: init_metrics_recorder(),
        gauge_task: None,
        feature_flags: Some(domain::FeatureFlags {
            chat_enabled: false,
            storage_readonly: false,
            maintenance_mode: false,
        }),
        ws_connection_permits: std::sync::Arc::new(tokio::sync::Semaphore::new(100)),
        ws_idle_timeout: std::time::Duration::from_secs(300),
        ws_shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
        ws_user_connections: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        ws_per_user_max: 10,
    }
}

#[tokio::test]
async fn auth_me_returns_200_when_auth_disabled() {
    let app = router_with_state(test_state_without_auth());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn auth_register_returns_404_when_auth_disabled() {
    let app = router_with_state(test_state_without_auth());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/register")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn auth_login_returns_404_when_auth_disabled() {
    let app = router_with_state(test_state_without_auth());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/login")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn auth_oauth_returns_404_when_auth_disabled() {
    let app = router_with_state(test_state_without_auth());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/oauth/google")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
