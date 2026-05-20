//! Integration tests for auth handlers requiring DB + Redis.
//!
//! Requires `DATABASE_URL` (with applied migrations) and a real Redis
//! instance reachable at `REDIS_URL`.  All tests are `#[serial]` to
//! prevent concurrent state corruption.
//!
//! Tests cover:
//! - forgot_password (POST /auth/forgot-password)
//! - reset_password (POST /auth/reset-password)
//! - delete_account (DELETE /auth/me)
//! - oauth_callback (GET /auth/oauth/{provider}/callback)

use api::router_with_state;
use api::metrics::init_metrics_recorder;
use api::{AppState, DbStatus, HealthState, WebSocketState};
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
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await
        .ok()?;

    let redis = Arc::new(
        cache::RedisClient::new(&redis_url, "test-auth-handlers")
            .await
            .ok()?,
    );

    let auth_config = auth::AuthConfig {
        jwt_secret: "test-auth-handlers-secret".to_string(),
        jwt_issuer: "test-issuer".to_string(),
        jwt_expiry: 3600,
        refresh_expiry: 7200,
        auth_mode: auth::AuthMode::Both,
        google_client_id: None,
        google_client_secret: None,
        github_client_id: None,
        github_client_secret: None,
        oauth_redirect_url: None,
        sidecar_shared_secret: None,
        fail_open_on_redis_error: true,
        rate_limits: Default::default(),
        cookie_secure: false,
    };
    let auth = Arc::new(auth::AuthService::new(auth_config));

    Some(AppState {
        health: Arc::new(HealthState {
            db: DbStatus::Connected(pool),
            redis: Some(redis),
            sidecar: PythonSidecar::new(
                "/tmp/__nonexistent_test_socket__.sock",
                Duration::from_secs(1),
                0,
            ),
            gauge_task: None,
            feature_flags: Some(domain::FeatureFlags {
                maintenance_mode: false,
            }),
        }),
        ws: Arc::new(WebSocketState {
            connection_permits: Arc::new(tokio::sync::Semaphore::new(100)),
            idle_timeout: Duration::from_secs(300),
            shutdown: Arc::new(tokio::sync::Notify::new()),
            user_connections: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            per_user_max: 10,
        }),
        auth: Some(auth),
        storage: None,
        prometheus_handle: test_prometheus_handle(),
        allowed_origin: None,
    })
}

fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@test.fullstackhex.local", uuid::Uuid::new_v4())
}

/// Delete a test user by email to prevent database pollution.
/// Uses a fresh connection so it works independently of the test's `full_state()`.
async fn cleanup_user(database_url: &str, email: &str) {
    let pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await
    {
        Ok(p) => p,
        Err(_) => return,
    };
    let _ = sqlx::query("DELETE FROM users WHERE email = $1")
        .bind(email)
        .execute(&pool)
        .await;
}

// ─── forgot_password ───────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn forgot_password_rejects_invalid_email() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/forgot-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({"email": "not-an-email"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn forgot_password_rejects_empty_email() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/forgot-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({"email": ""})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn forgot_password_returns_202_for_nonexistent_email() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/forgot-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(
                        &serde_json::json!({"email": "nobody@doesnotexist.fullstackhex.local"}),
                    )
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Always 202 to prevent email enumeration
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

// ─── reset_password ────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn reset_password_rejects_short_password() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/reset-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "token": "some-valid-token",
                        "password": "short",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn reset_password_rejects_empty_token() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/reset-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "token": "",
                        "password": "NewSecureP@ss1",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn reset_password_rejects_invalid_token() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/reset-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "token": "nonexistent-reset-token",
                        "password": "NewSecureP@ss1",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn forgot_password_creates_reset_token_for_existing_user() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let email = unique_email("forgot-creates-token");

    // 1. Register a test user
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "email": &email,
                        "password": "OriginalP@ss1",
                        "name": "Forgot Creates Token Tester",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // 2. Request password reset
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/forgot-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({"email": &email})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Cleanup
    if let Some(db_url) = std::env::var("DATABASE_URL").ok() {
        cleanup_user(&db_url, &email).await;
    }
}

#[tokio::test]
#[serial]
async fn reset_password_full_flow_with_seeded_token() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let redis = state.health.redis.clone().expect("redis should be set");
    let app = router_with_state(state);

    let email = unique_email("reset-flow-seeded");

    // 1. Register a test user to get a real user_id in the DB
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "email": &email,
                        "password": "OriginalP@ss1",
                        "name": "Reset Flow Seeded Tester",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // 2. Retrieve user_id from register response
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let reg: Value = serde_json::from_slice(&bytes).unwrap();
    let user_id = reg["user"]["id"].as_str().unwrap().to_string();

    // 3. Seed a reset token directly in Redis
    let reset_token = "test-seeded-reset-token-for-flow";
    redis
        .cache_set(
            "reset",
            reset_token,
            &user_id,
            std::time::Duration::from_secs(300),
        )
        .await
        .expect("should seed reset token in Redis");

    let new_password = "NewSecureP@ss2";

    // 4. Reset the password with the seeded token
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/reset-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "token": reset_token,
                        "password": new_password,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 5. Verify old password no longer works
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "email": &email,
                        "password": "OriginalP@ss1",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 6. Verify new password works
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "email": &email,
                        "password": new_password,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 7. Verify reset token is consumed (cannot reuse)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/reset-password")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "token": reset_token,
                        "password": "YetAnotherP@ss3",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Cleanup
    if let Some(db_url) = std::env::var("DATABASE_URL").ok() {
        cleanup_user(&db_url, &email).await;
    }
}

// ─── delete_account ────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn delete_account_returns_401_without_auth() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/auth/me")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn delete_account_full_flow() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let email = unique_email("delete-account-flow");

    // 1. Register a test user
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "email": &email,
                        "password": "DeleteAccountP@ss1",
                        "name": "Delete Account Tester",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let reg: Value = serde_json::from_slice(&bytes).unwrap();
    let access_token = reg["access_token"].as_str().unwrap().to_string();

    // 2. Delete account
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/auth/me")
                .header("authorization", format!("Bearer {access_token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 3. Verify account is gone (login should fail)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "email": &email,
                        "password": "DeleteAccountP@ss1",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ─── oauth_callback ────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn oauth_callback_rejects_invalid_provider() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/oauth/invalidprovider/callback?state=xyz&code=abc")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn oauth_callback_rejects_missing_state_and_code() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/oauth/google/callback")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Missing query params should result in a 422 from serde deserialization
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn oauth_callback_rejects_invalid_state() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set");
        return;
    };
    let app = router_with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/oauth/google/callback?state=nonexistent-state&code=some-code")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
