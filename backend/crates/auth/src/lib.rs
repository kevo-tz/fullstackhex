//! Authentication crate for FullStackHex.
//!
//! Provides JWT + sessions + OAuth authentication.
//! Auth validates in Rust only — Python sidecar gets auth via HMAC-signed headers.

pub mod csrf;
pub mod jwt;
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
}

/// Auth mode determines how authentication is extracted from requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    /// Session cookie only. CSRF required for state-changing endpoints.
    Cookie,
    /// Bearer JWT header only. No CSRF needed.
    Bearer,
    /// Both modes. Bearer takes precedence if present.
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

        let auth_mode = match std::env::var("AUTH_MODE").unwrap_or_else(|_| "both".to_string()).as_str() {
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


