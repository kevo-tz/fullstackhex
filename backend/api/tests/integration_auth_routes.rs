/// Integration tests verifying auth routes are absent when auth is not configured.
use api::router_with_state;
use api::test_helpers;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn test_state_without_auth() -> api::AppState {
    test_helpers::new_test_state()
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
