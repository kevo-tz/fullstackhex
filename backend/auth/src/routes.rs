//! Authentication routes.
//!
//! POST /auth/register, POST /auth/login, POST /auth/logout,
//! POST /auth/refresh, GET /auth/me,
//! GET /auth/oauth/{provider}, GET /auth/oauth/{provider}/callback

use super::AuthService;
use super::middleware::AuthUser;
use super::password;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use domain::error::ApiError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Shared state for auth routes.
#[derive(Clone)]
pub struct AuthState {
    pub auth: Arc<AuthService>,
    pub db: sqlx::PgPool,
    pub redis: Arc<cache::RedisClient>,
    pub oauth: Arc<super::oauth::OAuthService>,
}

/// Extract client IP from request headers (X-Forwarded-For, X-Real-IP fallback).
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Check rate limit for auth endpoints.
async fn check_rate_limit(
    redis: &cache::RedisClient,
    key: &str,
    window: Duration,
    max_requests: u64,
) -> Result<(), ApiError> {
    let result = redis.rate_limit_check(key, window, max_requests).await?;
    if !result.allowed {
        return Err(ApiError::RateLimited(format!(
            "Rate limit exceeded. Try again after {} seconds.",
            (result
                .reset_at
                .saturating_sub(domain::time::unix_timestamp_ms()))
                / 1000
        )));
    }
    Ok(())
}

/// Register request body.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

/// Login request body.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Token response.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub csrf_token: String,
    pub user: UserInfo,
}

/// User info in token response.
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub provider: String,
}

/// Validate registration request fields.
pub(crate) fn validate_registration(body: &RegisterRequest) -> Result<(), ApiError> {
    if body.email.is_empty() || !body.email.contains('@') {
        return Err(ApiError::ValidationError("Invalid email".to_string()));
    }
    if body.password.len() < 8 {
        return Err(ApiError::ValidationError(
            "Password must be at least 8 characters".to_string(),
        ));
    }
    if body.password.len() > 1024 {
        return Err(ApiError::ValidationError(
            "Password must be at most 1024 characters".to_string(),
        ));
    }
    Ok(())
}

/// POST /auth/register — create user with email/password, return JWT.
///
/// # Errors
///
/// Returns `ValidationError` if email is invalid, password is weak, or user already exists.
/// Returns `InternalError` on database or token generation failure.
pub async fn register(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Rate limit by IP (configurable, default: 5 per 15 minutes)
    let ip = client_ip(&headers);
    check_rate_limit(
        &state.redis,
        &format!("register:{ip}"),
        Duration::from_secs(state.auth.config.rate_limits.register_window_secs),
        state.auth.config.rate_limits.register_max,
    )
    .await?;

    validate_registration(&body)?;

    // Check if user exists
    let existing: Option<(String,)> = sqlx::query_as("SELECT id::text FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "database query failed");
            ApiError::InternalError("Internal server error".to_string())
        })?;

    if existing.is_some() {
        return Err(ApiError::Conflict("Email already registered".to_string()));
    }

    // Hash password
    let password_hash = password::hash_password(&body.password)?;

    // Insert user
    let user_id: (String,) = sqlx::query_as(
        "INSERT INTO users (email, name, provider, password_hash) VALUES ($1, $2, 'local', $3) RETURNING id::text",
    )
    .bind(&body.email)
    .bind(&body.name)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
            tracing::error!(error = %e, "database query failed");
            ApiError::InternalError("Internal server error".to_string())
        })?;

    // Create JWT
    let access_token =
        state
            .auth
            .jwt
            .create_token(&user_id.0, &body.email, body.name.as_deref(), "local")?;

    // Create refresh token in Redis
    let refresh_token = uuid::Uuid::new_v4().to_string();
    state
        .redis
        .cache_set(
            "refresh",
            &refresh_token,
            &user_id.0,
            std::time::Duration::from_secs(state.auth.config.refresh_expiry),
        )
        .await?;

    let jwt_expiry = state.auth.config.jwt_expiry;
    let refresh_expiry = state.auth.config.refresh_expiry;
    let mut headers = HeaderMap::new();
    super::cookies::set_cookie(
        &mut headers,
        "access_token",
        &access_token,
        jwt_expiry,
        true,
    )?;
    super::cookies::set_cookie(
        &mut headers,
        "refresh_token",
        &refresh_token,
        refresh_expiry,
        true,
    )?;
    let csrf_token = super::csrf::generate_csrf_token();
    super::cookies::set_cookie(&mut headers, "csrf_token", &csrf_token, jwt_expiry, false)?;

    let response = TokenResponse {
        access_token: access_token.clone(),
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth.config.jwt_expiry,
        csrf_token: csrf_token.clone(),
        user: UserInfo {
            id: user_id.0,
            email: body.email,
            name: body.name,
            provider: "local".to_string(),
        },
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApiError::InternalError("Time went backwards".to_string()))?
        .as_secs();
    let session = cache::session::Session {
        user_id: response.user.id.clone(),
        email: response.user.email.clone(),
        name: response.user.name.clone(),
        provider: response.user.provider.clone(),
        created_at: now,
    };
    let session_id = state
        .redis
        .session_create(&session, std::time::Duration::from_secs(jwt_expiry))
        .await?;
    super::cookies::set_cookie(&mut headers, "session", &session_id, jwt_expiry, true)?;

    Ok((StatusCode::CREATED, headers, Json(response)))
}

/// POST /auth/login — authenticate with email/password, return JWT.
///
/// # Errors
///
/// Returns `Unauthorized` for invalid credentials or rate-limited requests.
/// Returns `InternalError` on database or token generation failure.
pub async fn login(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let ip = client_ip(&headers);

    // Progressive brute-force backoff by IP (before rate limit check)
    match state.redis.backoff_check(&ip, "login").await {
        Ok(()) => {}
        Err(cache::CacheError::BackoffBlocked {
            remaining_secs,
            count,
            label,
        }) => {
            return Err(ApiError::RateLimited(format!(
                "Too many login attempts ({} failures). Try again in {} seconds ({} cooldown).",
                count, remaining_secs, label
            )));
        }
        Err(e) => {
            return Err(ApiError::InternalError(format!(
                "Backoff check failed: {e}"
            )));
        }
    }

    // Rate limit by email (configurable, default: 5 per 5 minutes)
    check_rate_limit(
        &state.redis,
        &format!("login:email:{}", body.email),
        Duration::from_secs(state.auth.config.rate_limits.login_email_window_secs),
        state.auth.config.rate_limits.login_email_max,
    )
    .await?;

    // Rate limit by IP (configurable, default: 10 per 5 minutes)
    check_rate_limit(
        &state.redis,
        &format!("login:ip:{ip}"),
        Duration::from_secs(state.auth.config.rate_limits.login_ip_window_secs),
        state.auth.config.rate_limits.login_ip_max,
    )
    .await?;

    // Find user
    #[allow(clippy::type_complexity)]
    let user: Option<(String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT id::text, email, name, provider, password_hash FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "database query failed");
        ApiError::InternalError("Internal server error".to_string())
    })?;

    let (user_id, email, name, provider, password_hash) = match user {
        Some(u) => u,
        None => {
            if let Err(e) = state.redis.backoff_increment(&ip, "login").await {
                tracing::warn!(ip = %ip, error = %e, "backoff_increment failed");
            }
            return Err(ApiError::Unauthorized("Invalid credentials".to_string()));
        }
    };

    // Verify password
    let hash = match password_hash {
        Some(h) => h,
        None => {
            if let Err(e) = state.redis.backoff_increment(&ip, "login").await {
                tracing::warn!(ip = %ip, error = %e, "backoff_increment failed");
            }
            return Err(ApiError::Unauthorized("Invalid credentials".to_string()));
        }
    };

    if !password::verify_password(&body.password, &hash)? {
        if let Err(e) = state.redis.backoff_increment(&ip, "login").await {
            tracing::warn!(ip = %ip, error = %e, "backoff_increment failed");
        }
        return Err(ApiError::Unauthorized("Invalid credentials".to_string()));
    }

    // Create JWT
    let access_token = state
        .auth
        .jwt
        .create_token(&user_id, &email, name.as_deref(), &provider)?;

    // Create refresh token in Redis
    let refresh_token = uuid::Uuid::new_v4().to_string();
    state
        .redis
        .cache_set(
            "refresh",
            &refresh_token,
            &user_id,
            std::time::Duration::from_secs(state.auth.config.refresh_expiry),
        )
        .await?;

    let jwt_expiry = state.auth.config.jwt_expiry;
    let refresh_expiry = state.auth.config.refresh_expiry;
    let csrf_token = super::csrf::generate_csrf_token();

    let response = TokenResponse {
        access_token: access_token.clone(),
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: jwt_expiry,
        csrf_token: csrf_token.clone(),
        user: UserInfo {
            id: user_id,
            email,
            name,
            provider,
        },
    };

    let mut headers = HeaderMap::new();
    super::cookies::set_cookie(
        &mut headers,
        "access_token",
        &response.access_token,
        jwt_expiry,
        true,
    )?;
    super::cookies::set_cookie(
        &mut headers,
        "refresh_token",
        &response.refresh_token,
        refresh_expiry,
        true,
    )?;
    super::cookies::set_cookie(&mut headers, "csrf_token", &csrf_token, jwt_expiry, false)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApiError::InternalError("Time went backwards".to_string()))?
        .as_secs();
    let session = cache::session::Session {
        user_id: response.user.id.clone(),
        email: response.user.email.clone(),
        name: response.user.name.clone(),
        provider: response.user.provider.clone(),
        created_at: now,
    };
    let session_id = state
        .redis
        .session_create(&session, std::time::Duration::from_secs(jwt_expiry))
        .await?;
    super::cookies::set_cookie(&mut headers, "session", &session_id, jwt_expiry, true)?;

    Ok((headers, Json(response)))
}

/// POST /auth/logout — destroy session, blacklist token, delete refresh token.
///
/// # Errors
///
/// Returns `InternalError` if Redis operations fail.
pub async fn logout(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    body: Option<Json<RefreshRequest>>,
) -> Result<impl IntoResponse, ApiError> {
    // Delete the refresh token from Redis so it cannot be reused
    if let Some(refresh_token) = body
        .as_ref()
        .map(|b| b.refresh_token.clone())
        .filter(|t| !t.is_empty())
    {
        state.redis.cache_delete("refresh", &refresh_token).await?;
    }

    // Blacklist the access token JTI so it cannot be reused
    let blacklist_ttl = std::time::Duration::from_secs(state.auth.config.jwt_expiry);
    state
        .redis
        .cache_set("blacklist", &auth_user.jti, &true, blacklist_ttl)
        .await?;

    // Destroy session if present (cookie auth mode)
    if let Some(ref session_id) = auth_user.session_id {
        // Best-effort: session might already be expired or destroyed
        if let Err(e) = state.redis.session_destroy(session_id).await {
            tracing::warn!(session = %session_id, error = %e, "session_destroy failed");
        }
    }

    tracing::info!(user_id = %auth_user.user_id, jti = %auth_user.jti, "user logged out");

    let mut headers = HeaderMap::new();
    let mut clear = |name: &str, http_only: bool| {
        let cookie = if http_only {
            format!("{name}=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax")
        } else {
            format!("{name}=; Path=/; Max-Age=0; SameSite=Lax")
        };
        headers.append(
            header::SET_COOKIE,
            cookie.parse().expect("valid Set-Cookie header"),
        );
    };
    clear("session", true);
    clear("access_token", true);
    clear("refresh_token", true);
    clear("csrf_token", false);

    Ok((StatusCode::NO_CONTENT, headers))
}

/// Refresh token request.
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    #[serde(default)]
    pub refresh_token: String,
}

/// POST /auth/refresh — refresh access token using refresh token.
///
/// # Errors
///
/// Returns `Unauthorized` if refresh token is invalid, expired, or not found.
/// Returns `InternalError` on database or token generation failure.
pub async fn refresh(
    State(state): State<AuthState>,
    headers: HeaderMap,
    body: Option<Json<RefreshRequest>>,
) -> Result<(HeaderMap, Json<TokenResponse>), ApiError> {
    // Read refresh token from JSON body first, fall back to cookie
    let refresh_token_str = body
        .as_ref()
        .map(|b| b.refresh_token.clone())
        .filter(|t| !t.is_empty())
        .or_else(|| {
            headers
                .get("cookie")
                .and_then(|v| v.to_str().ok())
                .and_then(|cookies| {
                    cookies.split(';').find_map(|c| {
                        let c = c.trim();
                        c.strip_prefix("refresh_token=").map(|s| s.to_string())
                    })
                })
        })
        .ok_or_else(|| ApiError::Unauthorized("Invalid or expired refresh token".to_string()))?;

    // Atomically read and delete the refresh token (prevents token family leak)
    let user_id = state
        .redis
        .refresh_token_rotate(&refresh_token_str)
        .await?
        .ok_or_else(|| {
            metrics::counter!("token_refresh_total", "status" => "failure").increment(1);
            ApiError::Unauthorized("Invalid or expired refresh token".to_string())
        })?;

    // Get user info
    let user: Option<(String, String, Option<String>, String)> =
        sqlx::query_as("SELECT id::text, email, name, provider FROM users WHERE id = $1::uuid")
            .bind(&user_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "database query failed");
                ApiError::InternalError("Internal server error".to_string())
            })?;

    let (user_id, email, name, provider) =
        user.ok_or_else(|| ApiError::Unauthorized("Invalid credentials".to_string()))?;

    // Create new access token
    let access_token = state
        .auth
        .jwt
        .create_token(&user_id, &email, name.as_deref(), &provider)
        .map_err(|_| ApiError::InternalError("Failed to create token".to_string()))?;

    // Create new refresh token
    let new_refresh_token = uuid::Uuid::new_v4().to_string();
    state
        .redis
        .cache_set(
            "refresh",
            &new_refresh_token,
            &user_id,
            std::time::Duration::from_secs(state.auth.config.refresh_expiry),
        )
        .await?;

    metrics::counter!("token_refresh_total", "status" => "success").increment(1);

    let mut resp_headers = HeaderMap::new();
    super::cookies::set_cookie(
        &mut resp_headers,
        "access_token",
        &access_token,
        state.auth.config.jwt_expiry,
        true,
    )?;
    super::cookies::set_cookie(
        &mut resp_headers,
        "refresh_token",
        &new_refresh_token,
        state.auth.config.refresh_expiry,
        true,
    )?;
    let csrf_token = super::csrf::generate_csrf_token();
    super::cookies::set_cookie(
        &mut resp_headers,
        "csrf_token",
        &csrf_token,
        state.auth.config.jwt_expiry,
        false,
    )?;

    Ok((
        resp_headers,
        Json(TokenResponse {
            access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: state.auth.config.jwt_expiry,
            csrf_token,
            user: UserInfo {
                id: user_id,
                email,
                name,
                provider,
            },
        }),
    ))
}

/// List configured OAuth providers from the auth config.
fn list_providers(config: &super::AuthConfig) -> Vec<&'static str> {
    let mut list: Vec<&str> = Vec::new();
    if config.google_client_id.is_some() {
        list.push("google");
    }
    if config.github_client_id.is_some() {
        list.push("github");
    }
    list
}

/// GET /auth/providers — list configured OAuth providers.
pub async fn providers(State(state): State<AuthState>) -> impl IntoResponse {
    Json(serde_json::json!({ "providers": list_providers(&state.auth.config) }))
}

/// GET /auth/me — return current user info.
///
/// # Errors
///
/// Returns `Unauthorized` if auth token is missing, invalid, or expired.
pub async fn me(auth_user: AuthUser) -> impl IntoResponse {
    Json(serde_json::json!({
        "user_id": auth_user.user_id,
        "email": auth_user.email,
        "name": auth_user.name,
        "provider": auth_user.provider,
    }))
}

/// GET /auth/oauth/{provider} — redirect to OAuth provider.
pub async fn oauth_redirect(
    State(state): State<AuthState>,
    Path(provider): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let provider = parse_provider(&provider)?;

    if !state.oauth.is_configured(&provider) {
        return Err(ApiError::ServiceUnavailable(format!(
            "{provider} OAuth not configured"
        )));
    }

    let redirect_url = state
        .auth
        .config
        .oauth_redirect_url
        .clone()
        .ok_or_else(|| ApiError::InternalError("OAUTH_REDIRECT_URL not configured".to_string()))?;

    let (url, csrf) = state.oauth.get_redirect_url(&provider, &redirect_url)?;

    // Store CSRF token in Redis with 10-minute TTL for callback validation
    state
        .redis
        .cache_set(
            "oauth_csrf",
            csrf.secret(),
            &provider.to_string(),
            std::time::Duration::from_secs(600),
        )
        .await?;

    Ok((StatusCode::FOUND, [(header::LOCATION, url)]))
}

/// OAuth callback query parameters.
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: String,
}

/// GET /auth/oauth/{provider}/callback — handle OAuth callback.
pub async fn oauth_callback(
    State(state): State<AuthState>,
    Path(provider): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let provider = parse_provider(&provider)?;

    // Validate CSRF state token
    let stored_provider: Option<String> = state.redis.cache_get("oauth_csrf", &query.state).await?;

    let stored_provider = stored_provider
        .ok_or_else(|| ApiError::Unauthorized("Invalid or expired OAuth state".to_string()))?;

    if stored_provider != provider.to_string() {
        return Err(ApiError::Unauthorized(
            "OAuth provider mismatch".to_string(),
        ));
    }

    // Delete the CSRF token (one-time use)
    state.redis.cache_delete("oauth_csrf", &query.state).await?;

    // Exchange code for access token
    let user_info = state
        .oauth
        .exchange_code(&provider, &query.code)
        .await
        .map_err(|_e| ApiError::Unauthorized("OAuth authentication failed".to_string()))?;

    // Find or create user (UPSERT to handle concurrent OAuth callbacks)
    let id = uuid::Uuid::new_v4().to_string();
    let user_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO users (id, email, name, provider, password_hash)
         VALUES ($1, $2, $3, $4, NULL)
         ON CONFLICT (email) DO UPDATE SET
           name = EXCLUDED.name
         RETURNING id::text",
    )
    .bind(&id)
    .bind(&user_info.email)
    .bind(&user_info.name)
    .bind(provider.to_string())
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "database query failed during OAuth callback");
        ApiError::InternalError("Internal server error".to_string())
    })?;

    // Create JWT
    let access_token = state.auth.jwt.create_token(
        &user_id,
        &user_info.email,
        user_info.name.as_deref(),
        &provider.to_string(),
    )?;

    let refresh_token = uuid::Uuid::new_v4().to_string();
    state
        .redis
        .cache_set(
            "refresh",
            &refresh_token,
            &user_id,
            std::time::Duration::from_secs(state.auth.config.refresh_expiry),
        )
        .await?;

    metrics::counter!("oauth_callbacks_total", "provider" => provider.to_string()).increment(1);

    let csrf_token = super::csrf::generate_csrf_token();
    let response = TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth.config.jwt_expiry,
        csrf_token,
        user: UserInfo {
            id: user_id,
            email: user_info.email,
            name: user_info.name,
            provider: provider.to_string(),
        },
    };

    Ok(Json(response))
}

fn parse_provider(s: &str) -> Result<super::oauth::OAuthProvider, ApiError> {
    match s.to_lowercase().as_str() {
        "google" => Ok(super::oauth::OAuthProvider::Google),
        "github" => Ok(super::oauth::OAuthProvider::GitHub),
        _ => Err(ApiError::ValidationError(format!(
            "Unknown OAuth provider: {s}"
        ))),
    }
}

#[cfg(test)]
mod route_tests {
    use super::*;

    #[test]
    fn client_ip_from_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());
        assert_eq!(client_ip(&headers), "192.168.1.1");
    }

    #[test]
    fn client_ip_from_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "10.0.0.2".parse().unwrap());
        assert_eq!(client_ip(&headers), "10.0.0.2");
    }

    #[test]
    fn client_ip_defaults_to_unknown() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), "unknown");
    }

    #[test]
    fn parse_provider_google() {
        assert!(matches!(
            parse_provider("google"),
            Ok(super::super::oauth::OAuthProvider::Google)
        ));
    }

    #[test]
    fn parse_provider_github() {
        assert!(matches!(
            parse_provider("github"),
            Ok(super::super::oauth::OAuthProvider::GitHub)
        ));
    }

    #[test]
    fn parse_provider_invalid() {
        assert!(parse_provider("invalid").is_err());
    }

    #[tokio::test]
    async fn me_handler_returns_user_info() {
        let auth_user = AuthUser {
            user_id: "user-123".to_string(),
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            provider: "local".to_string(),
            jti: "test-jti-1".to_string(),
            session_id: None,
        };
        let response = me(auth_user).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["user_id"], "user-123");
        assert_eq!(json["email"], "test@example.com");
        assert_eq!(json["name"], "Test User");
        assert_eq!(json["provider"], "local");
    }

    #[tokio::test]
    async fn me_handler_returns_user_info_without_name() {
        let auth_user = AuthUser {
            user_id: "user-456".to_string(),
            email: "anon@example.com".to_string(),
            name: None,
            provider: "google".to_string(),
            jti: "test-jti-2".to_string(),
            session_id: None,
        };
        let response = me(auth_user).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["user_id"], "user-456");
        assert_eq!(json["email"], "anon@example.com");
        assert!(json["name"].is_null());
        assert_eq!(json["provider"], "google");
    }

    #[test]
    fn validate_registration_rejects_empty_email() {
        let body = RegisterRequest {
            email: "".to_string(),
            password: "password123".to_string(),
            name: None,
        };
        let err = validate_registration(&body).unwrap_err();
        assert!(matches!(err, ApiError::ValidationError(_)));
    }

    #[test]
    fn validate_registration_rejects_email_without_at() {
        let body = RegisterRequest {
            email: "notanemail".to_string(),
            password: "password123".to_string(),
            name: None,
        };
        let err = validate_registration(&body).unwrap_err();
        assert!(matches!(err, ApiError::ValidationError(_)));
    }

    #[test]
    fn validate_registration_rejects_short_password() {
        let body = RegisterRequest {
            email: "a@b.com".to_string(),
            password: "short".to_string(),
            name: None,
        };
        let err = validate_registration(&body).unwrap_err();
        assert!(matches!(err, ApiError::ValidationError(_)));
    }

    #[test]
    fn validate_registration_accepts_valid_input() {
        let body = RegisterRequest {
            email: "user@example.com".to_string(),
            password: "password123".to_string(),
            name: Some("Test".to_string()),
        };
        assert!(validate_registration(&body).is_ok());
    }

    #[test]
    fn list_providers_empty_when_no_oauth() {
        let config = crate::AuthConfig {
            jwt_secret: "test".to_string(),
            jwt_issuer: "test".to_string(),
            jwt_expiry: 900,
            refresh_expiry: 604800,
            auth_mode: crate::AuthMode::Both,
            google_client_id: None,
            google_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
            oauth_redirect_url: None,
            sidecar_shared_secret: None,
            fail_open_on_redis_error: true,
            rate_limits: Default::default(),
        };
        let list = list_providers(&config);
        assert!(list.is_empty());
    }

    #[test]
    fn list_providers_google_only() {
        let config = crate::AuthConfig {
            jwt_secret: "test".to_string(),
            jwt_issuer: "test".to_string(),
            jwt_expiry: 900,
            refresh_expiry: 604800,
            auth_mode: crate::AuthMode::Both,
            google_client_id: Some("g-id".to_string()),
            google_client_secret: Some("g-secret".to_string()),
            github_client_id: None,
            github_client_secret: None,
            oauth_redirect_url: None,
            sidecar_shared_secret: None,
            fail_open_on_redis_error: true,
            rate_limits: Default::default(),
        };
        let list = list_providers(&config);
        assert_eq!(list, vec!["google"]);
    }

    #[test]
    fn list_providers_github_only() {
        let config = crate::AuthConfig {
            jwt_secret: "test".to_string(),
            jwt_issuer: "test".to_string(),
            jwt_expiry: 900,
            refresh_expiry: 604800,
            auth_mode: crate::AuthMode::Both,
            google_client_id: None,
            google_client_secret: None,
            github_client_id: Some("gh-id".to_string()),
            github_client_secret: Some("gh-secret".to_string()),
            oauth_redirect_url: None,
            sidecar_shared_secret: None,
            fail_open_on_redis_error: true,
            rate_limits: Default::default(),
        };
        let list = list_providers(&config);
        assert_eq!(list, vec!["github"]);
    }

    #[test]
    fn list_providers_both_providers() {
        let config = crate::AuthConfig {
            jwt_secret: "test".to_string(),
            jwt_issuer: "test".to_string(),
            jwt_expiry: 900,
            refresh_expiry: 604800,
            auth_mode: crate::AuthMode::Both,
            google_client_id: Some("g-id".to_string()),
            google_client_secret: Some("g-secret".to_string()),
            github_client_id: Some("gh-id".to_string()),
            github_client_secret: Some("gh-secret".to_string()),
            oauth_redirect_url: None,
            sidecar_shared_secret: None,
            fail_open_on_redis_error: true,
            rate_limits: Default::default(),
        };
        let list = list_providers(&config);
        assert_eq!(list, vec!["google", "github"]);
    }

    #[test]
    fn token_response_includes_csrf_token() {
        let resp = TokenResponse {
            access_token: "at".to_string(),
            refresh_token: "rt".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 900,
            csrf_token: "test-csrf-token".to_string(),
            user: UserInfo {
                id: "u1".to_string(),
                email: "a@b.com".to_string(),
                name: None,
                provider: "local".to_string(),
            },
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["csrf_token"], "test-csrf-token");
        assert_eq!(json["access_token"], "at");
        assert_eq!(json["user"]["email"], "a@b.com");
    }
}
