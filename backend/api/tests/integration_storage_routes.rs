use api::AppState;
use api::DbStatus;
/// Integration tests verifying storage routes are absent when storage is not configured.
use api::metrics::init_metrics_recorder;
use api::router_with_state;
use axum::http::{Request, StatusCode};
use py_sidecar::PythonSidecar;
use std::time::Duration;
use tower::ServiceExt;

fn test_state_without_storage() -> AppState {
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
    }
}

#[tokio::test]
async fn storage_download_returns_404_when_storage_disabled() {
    let app = router_with_state(test_state_without_storage());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/storage/test-file.txt")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn storage_list_returns_404_when_storage_disabled() {
    let app = router_with_state(test_state_without_storage());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/storage")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn storage_presign_returns_404_when_storage_disabled() {
    let app = router_with_state(test_state_without_storage());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/storage/presign")
                .method("POST")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
