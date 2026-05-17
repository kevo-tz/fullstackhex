/// Integration tests for auth middleware behavior.
///
/// Builds a mini-router with the real auth middleware and a protected handler
/// to verify 401/200 behaviour without requiring a database or Redis.
use auth::AuthConfig;
use auth::AuthMode;
use auth::AuthService;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::{Extension, Json, Router};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

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

/// A handler that requires an authenticated user.
async fn protected_handler(auth_user: auth::middleware::AuthUser) -> impl IntoResponse {
    Json(json!({
        "user_id": auth_user.user_id,
        "email": auth_user.email,
    }))
}

fn test_app_with_auth() -> Router {
    Router::new()
        .route("/test-protected", axum::routing::get(protected_handler))
        .layer(axum::middleware::from_fn(auth::middleware::auth_middleware))
        .layer(Extension(test_auth_service()))
}

#[tokio::test]
async fn protected_route_returns_401_without_auth() {
    let app = test_app_with_auth();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test-protected")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_returns_200_with_valid_jwt() {
    let auth = test_auth_service();
    let token = auth
        .jwt
        .create_token("user-123", "test@example.com", Some("Test User"), "local")
        .unwrap();

    let app = test_app_with_auth();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test-protected")
                .header("authorization", format!("Bearer {}", token))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["user_id"], "user-123");
    assert_eq!(v["email"], "test@example.com");
}

#[tokio::test]
async fn protected_route_returns_401_with_invalid_jwt() {
    let app = test_app_with_auth();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test-protected")
                .header("authorization", "Bearer invalid-token")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_returns_401_with_malformed_header() {
    let app = test_app_with_auth();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test-protected")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
