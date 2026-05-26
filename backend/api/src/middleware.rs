use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Maintenance mode middleware — returns 503 when FEATURE_MAINTENANCE is enabled,
/// unless the request targets a whitelisted route (health, metrics).
pub async fn maintenance_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let path = req.uri().path();
    let is_whitelisted = path == "/health"
        || path.starts_with("/health/")
        || path == "/metrics"
        || path.starts_with("/metrics/")
        || path == "/live"
        || path == "/auth/me";

    if !is_whitelisted
        && let Some(flags) = req.extensions().get::<domain::FeatureFlags>()
        && flags.maintenance_mode
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "{\"error\":\"maintenance mode\"}",
        )
            .into_response();
    }
    next.run(req).await
}

/// Middleware that injects a mock `AuthUser` using the `DEV_USER_ID` from
/// request extensions. Only applied on the notes sub-router when auth is
/// disabled but `DEV_USER_ID` is set.
pub async fn dev_user_middleware(
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let dev_user_id = req
        .extensions()
        .get::<crate::DevUserId>()
        .map(|d| d.0.clone());
    if let Some(uid) = dev_user_id {
        req.extensions_mut().insert(auth::middleware::AuthUser {
            user_id: uid,
            email: "dev@local.dev".into(),
            name: Some("Dev User".into()),
            provider: "dev".into(),
            jti: String::new(),
            session_id: None,
        });
    }
    next.run(req).await
}
