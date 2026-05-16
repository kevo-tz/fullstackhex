//! Integration tests for note CRUD routes.
//!
//! Requires `DATABASE_URL` to be set pointing to a test database with
//! migrations applied.  Tests that create stateful data are marked
//! `#[serial]` to prevent concurrent interference.

use api::AppState;
use api::DbStatus;
use api::metrics::init_metrics_recorder;
use api::router_with_state;
use auth::AuthConfig;
use auth::AuthMode;
use auth::AuthService;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use py_sidecar::PythonSidecar;
use serde_json::Value;
use serial_test::serial;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

fn test_prometheus_handle() -> metrics_exporter_prometheus::PrometheusHandle {
    init_metrics_recorder()
}

fn test_auth_service() -> Arc<AuthService> {
    let config = AuthConfig {
        jwt_secret: "test-secret-key-for-testing".to_string(),
        jwt_issuer: "test-issuer".to_string(),
        jwt_expiry: 900,
        refresh_expiry: 604800,
        auth_mode: AuthMode::Bearer,
        google_client_id: None,
        google_client_secret: None,
        github_client_id: None,
        github_client_secret: None,
        oauth_redirect_url: None,
        sidecar_shared_secret: None,
        fail_open_on_redis_error: true,
        rate_limits: Default::default(),
    };
    Arc::new(AuthService::new(config))
}

/// Connect to the database and build a minimal AppState with auth (no Redis).
async fn connect_db() -> Option<(AppState, PgPool)> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await
        .ok()?;

    let state = AppState {
        db: DbStatus::Connected(pool.clone()),
        redis: None,
        auth: Some(test_auth_service()),
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
    };

    Some((state, pool))
}

/// Register a test user in the database and return (user_id, token_string).
async fn create_test_user(
    pool: &PgPool,
    auth: &AuthService,
    email: &str,
) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, provider, password_hash) VALUES ($1::uuid, $2, 'local', 'test')",
    )
    .bind(user_id.to_string())
    .bind(email)
    .execute(pool)
    .await
    .expect("failed to create test user");

    let token = auth
        .jwt
        .create_token(&user_id.to_string(), email, None, "local")
        .expect("failed to create JWT");

    (user_id, token)
}

#[tokio::test]
#[serial]
async fn notes_create_list_get_delete() {
    let Some((state, pool)) = connect_db().await else {
        eprintln!("SKIP: DATABASE_URL not set or unreachable — skipping notes CRUD test");
        return;
    };

    let app = router_with_state(state);
    let auth = test_auth_service();
    let email = format!("notes-crud-{}@example.com", Uuid::new_v4());
    let (_user_id, token) = create_test_user(&pool, &auth, &email).await;

    // CREATE a note
    let body = serde_json::json!({
        "title": "Integration Test Note",
        "body": "Created during note CRUD integration test",
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/notes")
                .method("POST")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let note: Value = serde_json::from_slice(&bytes).unwrap();
    let note_id = note["id"].as_str().unwrap().to_string();
    assert_eq!(note["title"], "Integration Test Note");
    assert_eq!(note["body"], "Created during note CRUD integration test");
    assert!(note["created_at"].is_string());
    assert!(note["updated_at"].is_string());

    // LIST notes
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/notes")
                .method("GET")
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let list: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(list["total"].as_i64().unwrap_or(0) >= 1);
    assert_eq!(list["items"].as_array().unwrap().len() as i64, list["total"].as_i64().unwrap());

    // GET note by ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/notes/{note_id}"))
                .method("GET")
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let fetched: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(fetched["id"], note_id);
    assert_eq!(fetched["title"], "Integration Test Note");

    // DELETE note
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/notes/{note_id}"))
                .method("DELETE")
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // GET deleted note → 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/notes/{note_id}"))
                .method("GET")
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn notes_create_validation_title_required() {
    let Some((state, pool)) = connect_db().await else {
        eprintln!("SKIP: DATABASE_URL not set or unreachable");
        return;
    };

    let app = router_with_state(state);
    let auth = test_auth_service();
    let email = format!("notes-val-{}@example.com", Uuid::new_v4());
    let (_user_id, token) = create_test_user(&pool, &auth, &email).await;

    let body = serde_json::json!({
        "title": "   ",
        "body": "whitespace-only title",
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/notes")
                .method("POST")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[serial]
async fn notes_get_nonexistent_returns_404() {
    let Some((state, pool)) = connect_db().await else {
        eprintln!("SKIP: DATABASE_URL not set or unreachable");
        return;
    };

    let app = router_with_state(state);
    let auth = test_auth_service();
    let email = format!("notes-404-{}@example.com", Uuid::new_v4());
    let (_user_id, token) = create_test_user(&pool, &auth, &email).await;

    let fake_id = Uuid::new_v4().to_string();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&format!("/notes/{fake_id}"))
                .method("GET")
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn notes_list_returns_empty_for_new_user() {
    let Some((state, pool)) = connect_db().await else {
        eprintln!("SKIP: DATABASE_URL not set or unreachable");
        return;
    };

    let app = router_with_state(state);
    let auth = test_auth_service();
    let email = format!("notes-empty-{}@example.com", Uuid::new_v4());
    let (_user_id, token) = create_test_user(&pool, &auth, &email).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/notes")
                .method("GET")
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let list: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list["items"].as_array().unwrap().len(), 0);
    assert_eq!(list["total"].as_i64().unwrap_or(-1), 0);
}
