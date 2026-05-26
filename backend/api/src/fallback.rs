use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Fallback /auth/me when auth not configured.
/// Returns 200 with `{"status":"disabled"}` so browser doesn't log 404.
pub async fn auth_me_disabled() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        "{\"status\":\"disabled\"}",
    )
}

/// Fallback for notes endpoints when auth or database not configured.
pub async fn notes_fallback() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": "notes unavailable — auth or database not configured"})),
    )
}
