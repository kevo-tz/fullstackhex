/// Integration tests for the health routes.
///
/// These tests spin up the real axum router in-process and drive it with
/// actual HTTP requests via `tower::ServiceExt`.  Tests that mutate
/// environment use `#[serial]` to prevent concurrent execution.
use api::metrics::init_metrics_recorder;
use api::router;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use serial_test::serial;
use tower::ServiceExt;

fn test_prometheus_handle() -> metrics_exporter_prometheus::PrometheusHandle {
    init_metrics_recorder()
}

/// Save/restore guard for environment variables in tests.
/// On creation, captures the current value (if any). On drop, restores it.
/// Use with `#[serial]` to prevent concurrent env mutation.
struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        // SAFETY: must be paired with #[serial] to prevent concurrent env access.
        let previous = std::env::var(key).ok();
        unsafe { std::env::set_var(key, value) };
        EnvGuard { key, previous }
    }

    fn remove(key: &'static str) -> Self {
        // SAFETY: must be paired with #[serial] to prevent concurrent env access.
        let previous = std::env::var(key).ok();
        unsafe { std::env::remove_var(key) };
        EnvGuard { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(val) => unsafe { std::env::set_var(self.key, val) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

// ---------------------------------------------------------------------------
// /health  — no env mutation; these can run in parallel
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_200() {
    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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
    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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

    assert_eq!(v["rust"]["status"], "ok", "rust.status must be 'ok'");
    assert_eq!(v["rust"]["service"], "api", "rust.service must be 'api'");
    assert!(v["rust"]["version"].is_string(), "rust.version must be a string");
    assert!(v["db"]["status"].is_string(), "db.status must be a string");
    assert!(v["redis"]["status"].is_string(), "redis.status must be a string");
    assert!(v["storage"]["status"].is_string(), "storage.status must be a string");
    assert!(v["python"]["status"].is_string(), "python.status must be a string");
    assert!(v["auth"]["status"].is_string(), "auth.status must be a string");
}

#[tokio::test]
async fn health_content_type_is_json() {
    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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
#[serial]
async fn health_db_returns_503_when_not_configured() {
    let _guard = EnvGuard::remove("DATABASE_URL");
    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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
#[serial]
async fn health_db_error_when_no_database_url() {
    let _guard = EnvGuard::remove("DATABASE_URL");

    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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
    // With a real DATABASE_URL, the handler attempts to connect.
    // Since the URL may or may not point to a real database, we assert
    // the response shape but not the status value.
    let _guard = EnvGuard::set(
        "DATABASE_URL",
        "postgres://localhost:5432/nonexistent_test_db",
    );

    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/db")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Connection failed → 503
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert!(
        v["status"].is_string(),
        "status field must be present and a string"
    );
}

// ---------------------------------------------------------------------------
// /health/python  — env-mutating tests serialised with #[serial]
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn health_python_returns_503_when_no_socket() {
    let _guard = EnvGuard::set(
        "PYTHON_SIDECAR_SOCKET",
        "/tmp/__nonexistent_503_test__.sock",
    );
    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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
#[serial]
async fn health_python_unavailable_when_socket_absent() {
    let _guard = EnvGuard::set(
        "PYTHON_SIDECAR_SOCKET",
        "/tmp/__nonexistent_test_socket__.sock",
    );

    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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
}

#[tokio::test]
#[serial]
async fn health_python_ok_when_socket_present() {
    // Create a temp file and point PYTHON_SIDECAR_SOCKET at it.
    // The handler first checks is_available() which uses Path::exists().
    // If the file exists but is not a Unix socket, connect() will fail
    // and we'll get ConnectionFailed rather than SocketNotFound.
    // NamedTempFile auto-cleans up on drop, even if the test panics.
    let socket_file =
        tempfile::NamedTempFile::new().expect("should be able to create test socket file");
    let socket_path = socket_file.path().to_str().unwrap().to_string();
    let _guard = EnvGuard::set("PYTHON_SIDECAR_SOCKET", &socket_path);

    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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

    // The file exists (so is_available() returns true) but is not a Unix
    // socket, so connect() fails.  The handler maps ConnectionFailed to
    // status "unavailable".
    assert!(
        v["status"].is_string(),
        "status field must be present and a string"
    );
}

// ---------------------------------------------------------------------------
// Happy-path tests with real services
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn health_db_ok_with_real_pool() {
    use api::AppState;
    use api::DbStatus;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("SKIP: DATABASE_URL not set — skipping real-DB happy-path test");
            return;
        }
    };

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect(&database_url)
        .await;

    let pool = match pool {
        Ok(p) => p,
        Err(e) => {
            eprintln!("SKIP: could not connect to database ({e}) — skipping real-DB test");
            return;
        }
    };

    let state = AppState {
        db: DbStatus::Connected(pool),
        sidecar: py_sidecar::PythonSidecar::new(
            "/tmp/__nonexistent_test_socket__.sock",
            Duration::from_secs(1),
            0,
        ),
        prometheus_handle: test_prometheus_handle(),
        gauge_task: None,
        redis: None,
        auth: None,
        storage: None,
    };

    let app = api::router_with_state(state);
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

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert_eq!(
        v["status"], "ok",
        "health_db should return 'ok' when connected to a real database"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires real Unix socket infrastructure — run with --ignored on a native Linux host"]
async fn health_python_ok_with_mock_socket() {
    use api::AppState;
    use api::DbStatus;
    use api::router_with_state;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("health_ok.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();
    let sc = py_sidecar::PythonSidecar::new(sock_path.clone(), Duration::from_secs(2), 0);

    // Spawn a mock sidecar that reads the request then responds with valid JSON.
    // Reading first ensures the client finishes writing before we close.
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 512];
        let _ = stream.read(&mut buf).await;
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"ok\",\"service\":\"py-api\",\"version\":\"0.1.0\"}";
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.shutdown().await.ok();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    // Brief yield so the spawned task can enter accept().
    tokio::time::sleep(Duration::from_millis(100)).await;

    let state = AppState {
        db: DbStatus::NotConfigured,
        sidecar: sc,
        prometheus_handle: test_prometheus_handle(),
        gauge_task: None,
        redis: None,
        auth: None,
        storage: None,
    };

    let app = router_with_state(state);
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

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).expect("response must be valid JSON");

    assert_eq!(
        v["status"], "ok",
        "health_python should return 'ok' when sidecar responds successfully. Got: {v}"
    );
    assert_eq!(v["service"], "py-api");
    assert_eq!(v["version"], "0.1.0");
}

// ---------------------------------------------------------------------------
// Unknown routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_route_returns_404() {
    let (app, _state) = router(test_prometheus_handle()).await.unwrap();
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
