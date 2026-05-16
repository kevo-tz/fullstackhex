//! Integration tests for token refresh lifecycle.
//!
//! Requires `DATABASE_URL` (with applied migrations) and a real Redis
//! instance reachable at `REDIS_URL`.  All tests are `#[serial]` to
//! prevent concurrent state corruption.

use api::AppState;
use api::DbStatus;
use api::metrics::init_metrics_recorder;
use api::router_with_state;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use py_sidecar::PythonSidecar;
use serde_json::Value;
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

fn test_prometheus_handle() -> metrics_exporter_prometheus::PrometheusHandle {
    init_metrics_recorder()
}

/// Connect to both DB + Redis and build a fully-backed AppState.
/// Returns `None` when either service is unavailable (test is skipped).
async fn full_state() -> Option<AppState> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    let redis_url = std::env::var("REDIS_URL").ok()?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await
        .ok()?;

    let redis = Arc::new(
        cache::RedisClient::new(&redis_url, "test-refresh")
            .await
            .ok()?,
    );

    let auth_config = auth::AuthConfig {
        jwt_secret: "test-refresh-token-secret".to_string(),
        jwt_issuer: "test-issuer".to_string(),
        jwt_expiry: 60,
        refresh_expiry: 300,
        auth_mode: auth::AuthMode::Bearer,
        google_client_id: None,
        google_client_secret: None,
        github_client_id: None,
        github_client_secret: None,
        oauth_redirect_url: None,
        sidecar_shared_secret: None,
        fail_open_on_redis_error: true,
        rate_limits: Default::default(),
    };
    let auth = Arc::new(auth::AuthService::new(auth_config));

    Some(AppState {
        db: DbStatus::Connected(pool),
        redis: Some(redis),
        auth: Some(auth),
        storage: None,
        sidecar: PythonSidecar::new(
            "/tmp/__nonexistent_test_socket__.sock",
            Duration::from_secs(1),
            0,
        ),
        prometheus_handle: test_prometheus_handle(),
        gauge_task: None,
        feature_flags: Some(domain::FeatureFlags {
            chat_enabled: false,
            storage_readonly: false,
            maintenance_mode: false,
        }),
        ws_connection_permits: std::sync::Arc::new(tokio::sync::Semaphore::new(100)),
        ws_idle_timeout: Duration::from_secs(300),
        ws_shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
        ws_user_connections: std::sync::Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        ws_per_user_max: 10,
    })
}

#[tokio::test]
#[serial]
async fn refresh_token_lifecycle() {
    let Some(state) = full_state().await else {
        eprintln!(
            "SKIP: DATABASE_URL or REDIS_URL not set/unreachable — skipping token refresh test"
        );
        return;
    };

    let app = router_with_state(state);

    // 1. REGISTER a test user
    let register_body = serde_json::json!({
        "email": "refresh-lifecycle@example.com",
        "password": "SecureP@ss1",
        "name": "Refresh Tester",
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/register")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&register_body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "register should return 201"
    );

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let reg: Value = serde_json::from_slice(&bytes).unwrap();
    let access_token = reg["access_token"].as_str().unwrap().to_string();
    let refresh_token = reg["refresh_token"].as_str().unwrap().to_string();
    assert!(!access_token.is_empty());
    assert!(!refresh_token.is_empty());

    // 2. Use access token — GET /auth/me
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .header("authorization", format!("Bearer {access_token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "token should be valid");

    // 3. REFRESH the token
    let refresh_body = serde_json::json!({
        "refresh_token": refresh_token,
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/refresh")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&refresh_body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "refresh should return 200"
    );

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let refreshed: Value = serde_json::from_slice(&bytes).unwrap();
    let new_access = refreshed["access_token"].as_str().unwrap().to_string();
    let new_refresh = refreshed["refresh_token"].as_str().unwrap().to_string();
    assert!(!new_access.is_empty());
    assert!(!new_refresh.is_empty());
    assert_ne!(new_access, access_token, "new access token should differ");

    // 4. LOGOUT
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .method("POST")
                .header("authorization", format!("Bearer {new_access}"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "refresh_token": new_refresh,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "logout should return 200"
    );

    // 5. Use the OLD refresh token after logout → should fail
    let refresh_body = serde_json::json!({
        "refresh_token": new_refresh,
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/refresh")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&refresh_body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "refreshing after logout should fail"
    );
}

#[tokio::test]
#[serial]
async fn refresh_with_invalid_token_returns_401() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set/unreachable");
        return;
    };

    let app = router_with_state(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/refresh")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "refresh_token": "totally-fake-token",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "invalid refresh token should return 401"
    );
}

#[tokio::test]
#[serial]
async fn refresh_without_body_returns_422() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set/unreachable");
        return;
    };

    let app = router_with_state(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/refresh")
                .method("POST")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "missing refresh_token should return 401"
    );
}
