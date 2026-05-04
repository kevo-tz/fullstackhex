//! Authentication routes.
//!
//! POST /auth/register, POST /auth/login, POST /auth/logout,
//! POST /auth/refresh, GET /auth/me,
//! GET /auth/oauth/{provider}, GET /auth/oauth/{provider}/callback

use super::middleware::AuthUser;
use super::password;
use super::AuthService;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use domain::error::ApiError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Shared state for auth routes.
#[derive(Clone)]
pub struct AuthState {
    pub auth: Arc<AuthService>,
    pub db: sqlx::PgPool,
    pub redis: Arc<cache::RedisClient>,
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

/// POST /auth/register — create user with email/password, return JWT.
pub async fn register(
    State(state): State<AuthState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate input
    if body.email.is_empty() || !body.email.contains('@') {
        return Err(ApiError::ValidationError("Invalid email".to_string()));
    }
    if body.password.len() < 8 {
        return Err(ApiError::ValidationError(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Check if user exists
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(&body.email)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB error: {e}")))?;

    if existing.is_some() {
        return Err(ApiError::ValidationError(
            "Email already registered".to_string(),
        ));
    }

    // Hash password
    let password_hash = password::hash_password(&body.password)?;

    // Insert user
    let user_id: (String,) = sqlx::query_as(
        "INSERT INTO users (email, name, provider, password_hash) VALUES ($1, $2, 'local', $3) RETURNING id",
    )
    .bind(&body.email)
    .bind(&body.name)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::InternalError(format!("DB error: {e}")))?;

    // Create JWT
    let access_token = state.auth.jwt.create_token(
        &user_id.0,
        &body.email,
        body.name.as_deref(),
        "local",
    )?;

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

    let response = TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth.config.jwt_expiry,
        user: UserInfo {
            id: user_id.0,
            email: body.email,
            name: body.name,
            provider: "local".to_string(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// POST /auth/login — authenticate with email/password, return JWT.
pub async fn login(
    State(state): State<AuthState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Find user
    let user: Option<(String, String, Option<String>, String, Option<String>)> =
        sqlx::query_as(
            "SELECT id, email, name, provider, password_hash FROM users WHERE email = $1",
        )
        .bind(&body.email)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::InternalError(format!("DB error: {e}")))?;

    let (user_id, email, name, provider, password_hash) =
        user.ok_or_else(|| ApiError::Unauthorized("Invalid credentials".to_string()))?;

    // Verify password
    let hash = password_hash.ok_or_else(|| {
        ApiError::Unauthorized("Account uses OAuth login".to_string())
    })?;

    if !password::verify_password(&body.password, &hash)? {
        return Err(ApiError::Unauthorized("Invalid credentials".to_string()));
    }

    // Create JWT
    let access_token =
        state
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

    let response = TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth.config.jwt_expiry,
        user: UserInfo {
            id: user_id,
            email,
            name,
            provider,
        },
    };

    Ok(Json(response))
}

/// POST /auth/logout — destroy session, blacklist token, delete refresh token.
pub async fn logout(
    State(_state): State<AuthState>,
    auth_user: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    // TODO: blacklist the current access token JTI in Redis
    // TODO: destroy session in Redis

    tracing::info!(user_id = %auth_user.user_id, "user logged out");
    Ok(StatusCode::NO_CONTENT)
}

/// Refresh token request.
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// POST /auth/refresh — refresh access token using refresh token.
pub async fn refresh(
    State(state): State<AuthState>,
    Json(body): Json<RefreshRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Look up refresh token in Redis
    let user_id: Option<String> = state
        .redis
        .cache_get("refresh", &body.refresh_token)
        .await?;

    let user_id = user_id.ok_or_else(|| {
        ApiError::Unauthorized("Invalid or expired refresh token".to_string())
    })?;

    // Delete old refresh token (rotation)
    state
        .redis
        .cache_delete("refresh", &body.refresh_token)
        .await?;

    // Get user info
    let user: Option<(String, String, Option<String>, String)> =
        sqlx::query_as("SELECT id, email, name, provider FROM users WHERE id = $1")
            .bind(&user_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB error: {e}")))?;

    let (user_id, email, name, provider) =
        user.ok_or_else(|| ApiError::Unauthorized("User not found".to_string()))?;

    // Create new access token
    let access_token =
        state
            .auth
            .jwt
            .create_token(&user_id, &email, name.as_deref(), &provider)?;

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

    let response = TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth.config.jwt_expiry,
        user: UserInfo {
            id: user_id,
            email,
            name,
            provider,
        },
    };

    Ok(Json(response))
}

/// GET /auth/me — return current user info.
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
    let oauth_service = super::oauth::OAuthService::new(
        state.auth.config.google_client_id.clone(),
        state.auth.config.google_client_secret.clone(),
        state.auth.config.github_client_id.clone(),
        state.auth.config.github_client_secret.clone(),
    );

    if !oauth_service.is_configured(&provider) {
        return Err(ApiError::ServiceUnavailable(format!(
            "{provider} OAuth not configured"
        )));
    }

    let redirect_url = state
        .auth
        .config
        .oauth_redirect_url
        .clone()
        .unwrap_or_else(|| format!("http://localhost:8001/auth/oauth/{provider}/callback"));

    let (url, _csrf) = oauth_service.get_redirect_url(&provider, &redirect_url)?;
    Ok((
        StatusCode::FOUND,
        [(header::LOCATION, url)],
    ))
}

/// GET /auth/oauth/{provider}/callback — handle OAuth callback.
pub async fn oauth_callback(
    State(_state): State<AuthState>,
    Path(_provider): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    return Err::<axum::response::Response, _>(ApiError::InternalError("OAuth callback not yet implemented".to_string()));
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
