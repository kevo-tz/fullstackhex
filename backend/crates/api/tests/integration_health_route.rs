/// Integration tests for the health routes.
///
/// These tests spin up the real axum router in-process (no network socket
/// required) and drive it with actual HTTP requests via `tower::ServiceExt`.
/// This exercises routing, handler logic, and response serialisation end-to-end.
///
/// # Safety note
/// `std::env::set_var` / `remove_var` are unsafe in Rust 2024 because concurrent
/// mutation of env vars is UB.  Tests that mutate env vars are annotated with
/// `#[serial]` (via the `serial_test` crate) to prevent concurrent execution.
/// Each unsafe block has an explicit SAFETY comment.
use api::router;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use serial_test::serial;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// /health  — no env mutation; these can run in parallel
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_200() {
    let app = router();
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
async fn health_returns_json_with_status_ok() {
    let app = router();
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

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert_eq!(v["status"], "ok", "status field must be 'ok'");
    assert_eq!(v["service"], "api", "service field must be 'api'");
    assert!(v["version"].is_string(), "version field must be a string");
}

#[tokio::test]
async fn health_content_type_is_json() {
    let app = router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let ct = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    assert!(
        ct.contains("application/json"),
        "content-type must contain application/json, got: {ct}"
    );
}

// ---------------------------------------------------------------------------
// /health/db  — env-mutating tests serialised with #[serial]
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_db_returns_200() {
    let app = router();
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
#[serial]
async fn health_db_error_when_no_database_url() {
    // SAFETY: #[serial] ensures no other test mutates DATABASE_URL concurrently.
    unsafe { std::env::remove_var("DATABASE_URL") };

    let app = router();
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
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert_eq!(
        v["status"], "error",
        "status must be 'error' when DATABASE_URL is absent"
    );
    assert!(
        v["error"].is_string(),
        "error field must be present and a string"
    );
}

#[tokio::test]
#[serial]
async fn health_db_ok_when_database_url_set() {
    // The handler only checks whether DATABASE_URL is non-empty; no real connection.
    // SAFETY: #[serial] ensures no other test mutates DATABASE_URL concurrently.
    unsafe { std::env::set_var("DATABASE_URL", "postgres://localhost/testdb") };

    let app = router();
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
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert_eq!(
        v["status"], "ok",
        "status must be 'ok' when DATABASE_URL is set"
    );

    // SAFETY: restoring env; serialised by #[serial].
    unsafe { std::env::remove_var("DATABASE_URL") };
}

// ---------------------------------------------------------------------------
// /health/python  — env-mutating tests serialised with #[serial]
// ---------------------------------------------------------------------------

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

#[tokio::test]
#[serial]
async fn health_python_unavailable_when_socket_absent() {
    // SAFETY: #[serial] ensures no other test mutates PYTHON_SIDECAR_SOCKET concurrently.
    unsafe {
        std::env::set_var(
            "PYTHON_SIDECAR_SOCKET",
            "/tmp/__nonexistent_test_socket__.sock",
        )
    };

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

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert_eq!(
        v["status"], "unavailable",
        "status must be 'unavailable' when socket is absent"
    );
    assert!(
        v["error"].is_string(),
        "error field must be present and a string"
    );

    // SAFETY: restoring env; serialised by #[serial].
    unsafe { std::env::remove_var("PYTHON_SIDECAR_SOCKET") };
}

#[tokio::test]
#[serial]
async fn health_python_ok_when_socket_present() {
    // Create a temp file so the handler's `Path::exists()` returns true.
    let socket_path = "/tmp/__test_python_sidecar_present__.sock";
    std::fs::File::create(socket_path).expect("should be able to create test socket file");

    // SAFETY: #[serial] ensures no other test mutates PYTHON_SIDECAR_SOCKET concurrently.
    unsafe { std::env::set_var("PYTHON_SIDECAR_SOCKET", socket_path) };

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

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert_eq!(
        v["status"], "ok",
        "status must be 'ok' when socket exists"
    );

    let _ = std::fs::remove_file(socket_path);
    // SAFETY: restoring env; serialised by #[serial].
    unsafe { std::env::remove_var("PYTHON_SIDECAR_SOCKET") };
}

// ---------------------------------------------------------------------------
// Unknown routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_route_returns_404() {
    let app = router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
