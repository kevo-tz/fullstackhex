//! Integration tests for rate limiting at the HTTP layer.
//!
//! Verifies that the auth routes (login with wrong credentials) return
//! 429 Too Many Requests after exceeding the per-email threshold.
//! Requires `DATABASE_URL` and `REDIS_URL` to be set.

use api::AppState;
use api::DbStatus;
use api::metrics::init_metrics_recorder;
use api::router_with_state;
use axum::http::{Request, StatusCode};
use py_sidecar::PythonSidecar;
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

fn test_prometheus_handle() -> metrics_exporter_prometheus::PrometheusHandle {
    init_metrics_recorder()
}

async fn full_state() -> Option<AppState> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    let redis_url = std::env::var("REDIS_URL").ok()?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await
        .ok()?;

    let redis = Arc::new(cache::RedisClient::new(&redis_url, "test").await.ok()?);

    let auth_config = auth::AuthConfig {
        jwt_secret: "test-rate-limit-secret".to_string(),
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
async fn login_rate_limit_by_email() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set/unreachable — skipping rate limit test");
        return;
    };

    let app = router_with_state(state);
    let email = format!("ratelimit-{}@example.com", uuid::Uuid::new_v4());

    // login handler limits to 5 attempts per 5 minutes by email.
    // Send 6 wrong-password requests; expect the 6th to be 429.
    for i in 1..=6 {
        let body = serde_json::json!({
            "email": email,
            "password": "WrongPassword",
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/login")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            // Rate limiting kicked in — test passes
            return;
        }

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {i}: expected 401 before rate limit kicks in"
        );
    }

    panic!("rate limit was not triggered after 6 login attempts");
}

#[tokio::test]
#[serial]
async fn register_rate_limit_by_ip() {
    let Some(state) = full_state().await else {
        eprintln!("SKIP: DATABASE_URL or REDIS_URL not set/unreachable");
        return;
    };

    let app = router_with_state(state);

    // Each registration uses a unique email. The IP-based rate limit
    // (5 per hour) should eventually trigger.
    for i in 1..=6 {
        let body = serde_json::json!({
            "email": format!("ratelimit-reg-{i}-{}@example.com", uuid::Uuid::new_v4()),
            "password": "SecureP@ss1",
            "name": "Rate Limit Tester",
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/register")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return;
        }

        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "attempt {i}: expected 201 before rate limit kicks in"
        );
    }

    panic!("register rate limit was not triggered after 6 attempts");
}
