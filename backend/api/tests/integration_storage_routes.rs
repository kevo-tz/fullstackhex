/// Integration tests verifying storage routes are absent when storage is not configured.
use api::router_with_state;
use api::test_helpers;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn test_state_without_storage() -> api::AppState {
    test_helpers::new_test_state()
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
