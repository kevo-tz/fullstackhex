//! Authentication middleware and extractors.
//!
//! Provides `AuthLayer` middleware and `AuthUser` extractor for Axum.

use super::{AuthMode, AuthService};
use axum::Json;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::{Method, request::Parts};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::sync::Arc;

/// Authenticated user context extracted from the request.
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// Unique user identifier.
    pub user_id: String,
    /// User email address.
    pub email: String,
    /// Optional display name.
    pub name: Option<String>,
    /// Authentication provider (local, google, github).
    pub provider: String,
    /// JWT ID from the access token claims — used for logout blacklisting.
    pub jti: String,
    /// Session ID from the session cookie — None in bearer-only mode.
    pub session_id: Option<String>,
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
///
/// When Redis is available (injected via request extensions), the middleware
/// also checks whether the token's JTI has been blacklisted (e.g., after logout).
/// Blacklisted tokens are treated as unauthenticated — the request continues
/// without an AuthUser extension, and handlers using `AuthUser` extractor will
/// return 401.
pub async fn auth_middleware(
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    // Skip auth for public routes
    let path = req.uri().path();
    if path == "/live"
        || path.starts_with("/health")
        || path == "/metrics"
        || path.starts_with("/metrics/")
    {
        return next.run(req).await;
    }

    let auth_service = req.extensions().get::<Arc<AuthService>>().cloned();
    let redis = req.extensions().get::<Arc<cache::RedisClient>>().cloned();

    let Some(auth_service) = auth_service else {
        return next.run(req).await;
    };

    // Bearer extraction (sync, always available)
    let bearer_user = extract_bearer(&req, &auth_service);

    // Cookie auth prep: parse session cookie + validate CSRF (sync)
    let cookie_prep = cookie_auth_prepare(&req);

    // Determine the auth source and resolve the user.  Cookie auth needs Redis
    // I/O which produces a complex Future type; we box it to keep
    // axum::middleware::FromFn happy (it has a 16-type-parameter limit).
    let auth_user: Option<AuthUser> = match auth_service.config.auth_mode {
        AuthMode::Bearer => bearer_user,
        AuthMode::Both if bearer_user.is_some() => bearer_user,
        _ => {
            let (sess, rd) = match (cookie_prep, redis.as_ref()) {
                (Some(s), Some(r)) => (s, r),
                _ => return next.run(req).await,
            };
            match Box::pin(resolve_cookie_user(rd.clone(), sess)).await {
                Some(user) => Some(user),
                None => return next.run(req).await,
            }
        }
    };

    // JWT blacklist check
    if let (Some(user), Some(redis)) = (&auth_user, &redis) {
        let is_blacklisted: Option<bool> = match redis
            .cache_get("blacklist", &user.jti)
            .await
        {
            Ok(Some(v)) => Some(v),
            Ok(None) => Some(false),
            Err(e) => {
                tracing::warn!(
                    jti = %user.jti,
                    error = %e,
                    "blacklist check failed — Redis error"
                );
                None
            }
        };
        match is_blacklisted {
            Some(true) => {
                tracing::debug!(
                    jti = %user.jti,
                    user_id = %user.user_id,
                    "blacklisted token rejected"
                );
                return next.run(req).await;
            }
            None if auth_service.config.fail_open_on_redis_error => {
                tracing::warn!(
                    jti = %user.jti,
                    "blacklist check failed — allowing request (Redis unavailable, fail-open)"
                );
            }
            None => {
                tracing::warn!(
                    jti = %user.jti,
                    "blacklist check failed — rejecting request (fail-closed)"
                );
                return next.run(req).await;
            }
            Some(false) => {}
        }
    }

    if let Some(user) = auth_user {
        req.extensions_mut().insert(user);
    }

    next.run(req).await
}

/// Extract and validate a Bearer token from the Authorization header.
pub(crate) fn extract_bearer(
    req: &axum::http::Request<axum::body::Body>,
    auth_service: &AuthService,
) -> Option<AuthUser> {
    let header = req.headers().get("authorization")?;
    let value = header.to_str().ok()?;
    // Case-insensitive Bearer prefix per RFC 9110 Section 11.6.2
    let token = value
        .strip_prefix("bearer ")
        .or_else(|| value.strip_prefix("Bearer "))?;

    let claims = auth_service.jwt.validate_token(token).ok()?;
    Some(AuthUser {
        user_id: claims.sub,
        email: claims.email,
        name: claims.name,
        provider: claims.provider,
        jti: claims.jti,
        session_id: None, // bearer auth has no session
    })
}

/// Sync preparation: parse session cookie and validate CSRF.
/// Returns the session_id if everything passes.
fn cookie_auth_prepare(req: &axum::http::Request<axum::body::Body>) -> Option<String> {
    let session_id = req
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix("session=")
            })
        })?;

    if session_id.is_empty() {
        return None;
    }

    // CSRF validation for state-changing methods
    let method = req.method().clone();
    if method == Method::POST
        || method == Method::PUT
        || method == Method::DELETE
        || method == Method::PATCH
    {
        let csrf_header = req
            .headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let csrf_cookie = req
            .headers()
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies.split(';').find_map(|c| {
                    let c = c.trim();
                    c.strip_prefix("csrf_token=")
                })
            })
            .unwrap_or("");

        if !super::csrf::validate_csrf_token(csrf_cookie, csrf_header) {
            return None;
        }
    }

    Some(session_id.to_string())
}

/// Async: Resolve a cookie session via Redis lookup + JWT validation.
async fn resolve_cookie_user(
    redis: Arc<cache::RedisClient>,
    session_id: String,
) -> Option<AuthUser> {
    let session: Option<cache::session::Session> = redis
        .cache_get("session", &session_id)
        .await
        .unwrap_or(None);

    let session = session?;

    Some(AuthUser {
        user_id: session.user_id,
        email: session.email,
        name: session.name,
        provider: session.provider,
        jti: String::new(),
        session_id: Some(session_id),
    })
}

/// Compute HMAC-SHA256 signature for forwarding auth headers to Python sidecar.
///
/// Signs a JSON payload: `{"user_id":"...","email":"...","name":"..."}` sorted by key,
/// using the shared secret. Returns an error if the secret is empty.
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
    let payload = serde_json::json!({
        "user_id": user_id,
        "email": email,
        "name": name,
    })
    .to_string();
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
            fail_open_on_redis_error: true,
            rate_limits: Default::default(),
        };
        AuthService::new(config)
    }

    #[test]
    fn hmac_roundtrip() {
        let secret = "test-shared-secret";
        let sig = compute_auth_signature(secret, "user-123", "test@example.com", "Test").unwrap();
        assert!(verify_auth_signature(
            secret,
            "user-123",
            "test@example.com",
            "Test",
            &sig
        ));
    }

    #[test]
    fn hmac_wrong_secret_fails() {
        let sig = compute_auth_signature("secret1", "user-123", "a@b.com", "T").unwrap();
        assert!(!verify_auth_signature(
            "secret2", "user-123", "a@b.com", "T", &sig
        ));
    }

    #[test]
    fn hmac_wrong_payload_fails() {
        let sig = compute_auth_signature("secret", "user-123", "a@b.com", "T").unwrap();
        assert!(!verify_auth_signature(
            "secret", "user-456", "a@b.com", "T", &sig
        ));
    }

    #[test]
    fn hmac_empty_secret_fails() {
        let result = compute_auth_signature("", "user-123", "a@b.com", "T");
        assert!(result.is_err());
    }

    #[test]
    fn extract_bearer_missing_header() {
        let req = axum::http::Request::builder()
            .uri("/")
            .body(axum::body::Body::empty())
            .unwrap();
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
        let token = auth
            .jwt
            .create_token("u1", "a@b.com", None, "local")
            .unwrap();
        let req = axum::http::Request::builder()
            .uri("/")
            .header("authorization", format!("Bearer {}", token))
            .body(axum::body::Body::empty())
            .unwrap();
        let user = extract_bearer(&req, &auth).unwrap();
        assert_eq!(user.user_id, "u1");
        assert_eq!(user.email, "a@b.com");
        assert!(!user.jti.is_empty(), "jti should be populated");
        assert!(user.session_id.is_none(), "bearer auth has no session");
    }

    #[test]
    fn compute_auth_signature_sidecar_secret_roundtrip() {
        let sig = compute_auth_signature("my-shared-secret", "user-1", "a@b.com", "Alice").unwrap();
        assert!(verify_auth_signature(
            "my-shared-secret",
            "user-1",
            "a@b.com",
            "Alice",
            &sig,
        ));
        assert!(!verify_auth_signature(
            "my-shared-secret",
            "user-1",
            "a@b.com",
            "Eve",
            &sig,
        ));
    }
}
