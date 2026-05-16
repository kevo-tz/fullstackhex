//! Authentication crate for FullStackHex.
//!
//! Provides JWT + sessions + OAuth authentication.
//! Auth validates in Rust only — Python sidecar gets auth via HMAC-signed headers.

pub mod cookies;
pub mod csrf;
pub mod jwt;
pub mod metrics;
pub mod middleware;
pub mod oauth;
pub mod password;
pub mod routes;

/// Authentication configuration.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT secret for signing tokens.
    pub jwt_secret: String,
    /// JWT issuer claim.
    pub jwt_issuer: String,
    /// Access token expiry in seconds (default: 900 = 15min).
    pub jwt_expiry: u64,
    /// Refresh token expiry in seconds (default: 604800 = 7 days).
    pub refresh_expiry: u64,
    /// Auth mode: cookie, bearer, or both.
    pub auth_mode: AuthMode,
    /// Google OAuth client ID.
    pub google_client_id: Option<String>,
    /// Google OAuth client secret.
    pub google_client_secret: Option<String>,
    /// GitHub OAuth client ID.
    pub github_client_id: Option<String>,
    /// GitHub OAuth client secret.
    pub github_client_secret: Option<String>,
    /// OAuth redirect URL.
    pub oauth_redirect_url: Option<String>,
    /// HMAC shared secret for Python sidecar trust.
    pub sidecar_shared_secret: Option<String>,
    /// When true (default), allow requests through when blacklist check fails (Redis unavailable).
    /// When false, reject the request if the blacklist check cannot complete.
    pub fail_open_on_redis_error: bool,
}

/// Auth mode determines how authentication is extracted from requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    /// Session cookie only. CSRF required for state-changing endpoints.
    Cookie,
    /// Bearer JWT header only. No CSRF needed.
    Bearer,
    /// Both cookie and bearer auth. Bearer takes precedence when both are present.
    ///
    /// # Security considerations
    ///
    /// When both a cookie and a Bearer token are sent, the Bearer token is used.
    /// This means CSRF protection is bypassed for requests that include a Bearer
    /// header. This is safe because:
    /// 1. Bearer tokens can't be sent cross-origin without JS access
    /// 2. The `SameSite=Lax` cookie attribute provides additional CSRF protection
    /// 3. Cookie-auth requests still require CSRF validation for state-changing methods
    Both,
}

impl AuthConfig {
    /// Load auth config from environment variables.
    pub fn from_env() -> Option<Self> {
        let jwt_secret = std::env::var("JWT_SECRET").ok()?;
        if jwt_secret.is_empty() || jwt_secret == "CHANGE_ME" {
            tracing::warn!("JWT_SECRET not set or is CHANGE_ME — auth disabled");
            return None;
        }

        let auth_mode = match std::env::var("AUTH_MODE")
            .unwrap_or_else(|_| "both".to_string())
            .as_str()
        {
            "cookie" => AuthMode::Cookie,
            "bearer" => AuthMode::Bearer,
            _ => AuthMode::Both,
        };

        Some(Self {
            jwt_secret,
            jwt_issuer: std::env::var("JWT_ISSUER").unwrap_or_else(|_| "fullstackhex".to_string()),
            jwt_expiry: std::env::var("JWT_EXPIRY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(900),
            refresh_expiry: std::env::var("JWT_REFRESH_EXPIRY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(604800),
            auth_mode,
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").ok(),
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").ok(),
            github_client_id: std::env::var("GITHUB_CLIENT_ID").ok(),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET").ok(),
            oauth_redirect_url: std::env::var("OAUTH_REDIRECT_URL").ok(),
            sidecar_shared_secret: std::env::var("SIDECAR_SHARED_SECRET")
                .ok()
                .filter(|s| !s.is_empty()),
            fail_open_on_redis_error: std::env::var("AUTH_FAIL_OPEN_ON_REDIS_ERROR")
                .ok()
                .and_then(|v| v.parse::<bool>().ok())
                .unwrap_or(true),
        })
    }
}

/// Authentication service combining JWT, sessions, and OAuth.
pub struct AuthService {
    pub config: AuthConfig,
    pub jwt: jwt::JwtService,
}

impl AuthService {
    /// Create a new auth service from config.
    pub fn new(config: AuthConfig) -> Self {
        let jwt = jwt::JwtService::new(
            config.jwt_secret.clone(),
            config.jwt_issuer.clone(),
            config.jwt_expiry,
        );
        Self { config, jwt }
    }

    /// Create from environment variables. Returns None if auth is not configured.
    pub fn from_env() -> Option<Self> {
        let config = AuthConfig::from_env()?;
        Some(Self::new(config))
    }
}

#[cfg(test)]
mod proptests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_config_from_env_env_variants() {
        let secret_old = std::env::var("JWT_SECRET").ok();
        let mode_old = std::env::var("AUTH_MODE").ok();

        // 1. Missing JWT_SECRET => None
        unsafe { std::env::remove_var("JWT_SECRET") };
        assert!(AuthConfig::from_env().is_none());

        // 2. CHANGE_ME => None
        unsafe { std::env::set_var("JWT_SECRET", "CHANGE_ME") };
        assert!(AuthConfig::from_env().is_none());

        // 3. Valid secret => Some(...)
        unsafe {
            std::env::set_var("JWT_SECRET", "test-secret-for-tests");
            std::env::set_var("AUTH_MODE", "bearer");
        }
        let config = AuthConfig::from_env();
        assert!(config.is_some());
        let c = config.unwrap();
        assert_eq!(c.jwt_secret, "test-secret-for-tests");
        assert_eq!(c.auth_mode, AuthMode::Bearer);
        assert_eq!(c.jwt_issuer, "fullstackhex");
        assert_eq!(c.jwt_expiry, 900);
        assert_eq!(c.refresh_expiry, 604800);

        if let Some(v) = secret_old {
            unsafe { std::env::set_var("JWT_SECRET", v) };
        } else {
            unsafe { std::env::remove_var("JWT_SECRET") };
        }
        if let Some(v) = mode_old {
            unsafe { std::env::set_var("AUTH_MODE", v) };
        } else {
            unsafe { std::env::remove_var("AUTH_MODE") };
        }
    }

    #[test]
    fn auth_service_new_creates_jwt_service() {
        let config = AuthConfig {
            jwt_secret: "my-secret".into(),
            jwt_issuer: "my-issuer".into(),
            jwt_expiry: 600,
            refresh_expiry: 3600,
            auth_mode: AuthMode::Both,
            google_client_id: None,
            google_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
            oauth_redirect_url: None,
            sidecar_shared_secret: None,
            fail_open_on_redis_error: true,
        };
        let svc = AuthService::new(config);
        let token = svc
            .jwt
            .create_token("u1", "test@test.com", None, "local")
            .unwrap();
        let claims = svc.jwt.validate_token(&token).unwrap();
        assert_eq!(claims.iss, "my-issuer");
        assert_eq!(claims.sub, "u1");
    }

    #[test]
    fn auth_mode_display_and_debug() {
        assert_eq!(format!("{:?}", AuthMode::Cookie), "Cookie");
        assert_eq!(format!("{:?}", AuthMode::Bearer), "Bearer");
        assert_eq!(format!("{:?}", AuthMode::Both), "Both");
    }

    #[test]
    fn auth_mode_partial_eq() {
        assert_eq!(AuthMode::Cookie, AuthMode::Cookie);
        assert_ne!(AuthMode::Cookie, AuthMode::Bearer);
    }
}
