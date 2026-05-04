//! Authentication middleware and extractors.
//!
//! Provides `AuthLayer` middleware and `AuthUser` extractor for Axum.

use super::{AuthMode, AuthService};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;

/// Authenticated user context extracted from the request.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub email: String,
    pub name: Option<String>,
    pub provider: String,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or(AuthRejection::Unauthorized)
    }
}

/// Rejection type for auth extraction failures.
#[derive(Debug)]
pub enum AuthRejection {
    Unauthorized,
    Forbidden,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AuthRejection::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Authentication required",
            ),
            AuthRejection::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN", "Access denied"),
        };
        let body = serde_json::json!({ "error": { "code": code, "message": message } });
        (status, Json(body)).into_response()
    }
}

/// Axum middleware that extracts and validates authentication.
///
/// Based on `AUTH_MODE`:
/// - `cookie`: extract session cookie only
/// - `bearer`: extract Authorization header only
/// - `both`: bearer takes precedence, cookie is fallback
pub async fn auth_middleware(
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let auth_service = req
        .extensions()
        .get::<Arc<AuthService>>()
        .cloned();

    let Some(auth_service) = auth_service else {
        // No auth service — pass through (auth disabled)
        return next.run(req).await;
    };

    let auth_user = extract_auth_user(&req, &auth_service);

    if let Some(user) = auth_user {
        req.extensions_mut().insert(user);
    }
    // If no auth user, the request continues — individual handlers
    // can use AuthUser extractor to require authentication.

    next.run(req).await
}

/// Extract auth user from request based on auth mode.
fn extract_auth_user(
    req: &axum::http::Request<axum::body::Body>,
    auth_service: &AuthService,
) -> Option<AuthUser> {
    match auth_service.config.auth_mode {
        AuthMode::Bearer => extract_bearer(req, auth_service),
        AuthMode::Cookie => extract_cookie(req, auth_service),
        AuthMode::Both => {
            // Bearer takes precedence
            extract_bearer(req, auth_service)
                .or_else(|| extract_cookie(req, auth_service))
        }
    }
}

/// Extract and validate a Bearer token from the Authorization header.
fn extract_bearer(
    req: &axum::http::Request<axum::body::Body>,
    auth_service: &AuthService,
) -> Option<AuthUser> {
    let header = req.headers().get("authorization")?;
    let value = header.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;

    let claims = auth_service.jwt.validate_token(token).ok()?;
    Some(AuthUser {
        user_id: claims.sub,
        email: claims.email,
        name: claims.name,
        provider: claims.provider,
    })
}

/// Extract auth from a session cookie.
fn extract_cookie(
    _req: &axum::http::Request<axum::body::Body>,
    _auth_service: &AuthService,
) -> Option<AuthUser> {
    // TODO: Implement cookie-based session lookup from Redis
    // For now, cookie auth is a stub — bearer is the primary path
    None
}

/// Compute HMAC-SHA256 signature for forwarding auth headers to Python sidecar.
///
/// Signs: "{user_id}|{email}|{name}" using the shared secret.
/// Returns an error if the secret is empty (which would panic Hmac::new_from_slice).
pub fn compute_auth_signature(
    secret: &str,
    user_id: &str,
    email: &str,
    name: &str,
) -> Result<String, domain::error::ApiError> {
    if secret.is_empty() {
        return Err(domain::error::ApiError::InternalError(
            "SIDECAR_SHARED_SECRET is empty".to_string(),
        ));
    }
    let payload = format!("{user_id}|{email}|{name}");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| domain::error::ApiError::InternalError(format!("HMAC init failed: {e}")))?;
    mac.update(payload.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Verify an HMAC-SHA256 signature from the Python sidecar.
pub fn verify_auth_signature(
    secret: &str,
    user_id: &str,
    email: &str,
    name: &str,
    signature: &str,
) -> bool {
    let Ok(expected) = compute_auth_signature(secret, user_id, email, name) else {
        return false;
    };
    // Constant-time comparison
    if expected.len() != signature.len() {
        return false;
    }
    expected
        .bytes()
        .zip(signature.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthConfig;

    fn test_auth_service(mode: AuthMode) -> AuthService {
        let config = AuthConfig {
            jwt_secret: "test-secret-key-for-testing".to_string(),
            jwt_issuer: "test-issuer".to_string(),
            jwt_expiry: 900,
            refresh_expiry: 604800,
            auth_mode: mode,
            google_client_id: None,
            google_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
            oauth_redirect_url: None,
            sidecar_shared_secret: None,
        };
        AuthService::new(config)
    }

    #[test]
    fn hmac_roundtrip() {
        let secret = "test-shared-secret";
        let sig = compute_auth_signature(secret, "user-123", "test@example.com", "Test").unwrap();
        assert!(verify_auth_signature(secret, "user-123", "test@example.com", "Test", &sig));
    }

    #[test]
    fn hmac_wrong_secret_fails() {
        let sig = compute_auth_signature("secret1", "user-123", "a@b.com", "T").unwrap();
        assert!(!verify_auth_signature("secret2", "user-123", "a@b.com", "T", &sig));
    }

    #[test]
    fn hmac_wrong_payload_fails() {
        let sig = compute_auth_signature("secret", "user-123", "a@b.com", "T").unwrap();
        assert!(!verify_auth_signature("secret", "user-456", "a@b.com", "T", &sig));
    }

    #[test]
    fn hmac_empty_secret_fails() {
        let result = compute_auth_signature("", "user-123", "a@b.com", "T");
        assert!(result.is_err());
    }

    #[test]
    fn extract_bearer_missing_header() {
        let req = axum::http::Request::builder().uri("/").body(axum::body::Body::empty()).unwrap();
        let auth = test_auth_service(AuthMode::Bearer);
        assert!(extract_bearer(&req, &auth).is_none());
    }

    #[test]
    fn extract_bearer_invalid_token() {
        let req = axum::http::Request::builder()
            .uri("/")
            .header("authorization", "Bearer invalid-token")
            .body(axum::body::Body::empty())
            .unwrap();
        let auth = test_auth_service(AuthMode::Bearer);
        assert!(extract_bearer(&req, &auth).is_none());
    }

    #[test]
    fn extract_bearer_valid_token() {
        let auth = test_auth_service(AuthMode::Bearer);
        let token = auth.jwt.create_token("u1", "a@b.com", None, "local").unwrap();
        let req = axum::http::Request::builder()
            .uri("/")
            .header("authorization", format!("Bearer {}", token))
            .body(axum::body::Body::empty())
            .unwrap();
        let user = extract_bearer(&req, &auth).unwrap();
        assert_eq!(user.user_id, "u1");
        assert_eq!(user.email, "a@b.com");
    }

    #[test]
    fn extract_auth_user_both_prefers_bearer() {
        let auth = test_auth_service(AuthMode::Both);
        let token = auth.jwt.create_token("u1", "a@b.com", None, "local").unwrap();
        let req = axum::http::Request::builder()
            .uri("/")
            .header("authorization", format!("Bearer {}", token))
            .body(axum::body::Body::empty())
            .unwrap();
        let user = extract_auth_user(&req, &auth).unwrap();
        assert_eq!(user.user_id, "u1");
    }
}
