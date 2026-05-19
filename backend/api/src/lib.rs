use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Extension, FromRef, State},
    http::Request,
    middleware,
    routing::get,
};
use metrics_exporter_prometheus::PrometheusHandle;
use py_sidecar::PythonSidecar;
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

pub mod live;
pub mod metrics;

use live::{LiveEvent, broadcast_event};
pub mod notes;

/// Max length for health error details broadcast to WS clients.
const MAX_DETAIL_LENGTH: usize = 500;

/// Parse an env var, logging a warning on parse failure before returning default.
fn parse_env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    match std::env::var(key) {
        Ok(val) => match val.parse::<T>() {
            Ok(parsed) => parsed,
            Err(_) => {
                tracing::warn!(key = %key, value = %val, "failed to parse env var, using default");
                default
            }
        },
        Err(std::env::VarError::NotPresent) => default,
        Err(e) => {
            tracing::warn!(key = %key, error = %e, "error reading env var, using default");
            default
        }
    }
}

/// Small helper wrapping sqlx errors into ApiError with logging + metrics.
fn db_err(e: sqlx::Error, context: &'static str) -> domain::error::ApiError {
    tracing::warn!(error = %e, "{context}");
    ::metrics::counter!("notes_query_errors_total").increment(1);
    domain::error::ApiError::InternalError(context.into())
}

/// Newtype to carry DEV_USER_ID via request extensions.
#[derive(Clone)]
struct DevUserId(String);

/// Status of the database connection pool.
#[derive(Clone)]
pub enum DbStatus {
    NotConfigured,
    Connected(PgPool),
    ConnectionFailed(String),
}

/// Health-check related state.
#[derive(Clone)]
pub struct HealthState {
    pub db: DbStatus,
    pub redis: Option<Arc<cache::RedisClient>>,
    pub sidecar: PythonSidecar,
    pub gauge_task: Option<tokio::task::AbortHandle>,
    pub feature_flags: Option<domain::FeatureFlags>,
}

impl HealthState {
    /// Returns a reference to the PgPool, or an `ApiError::ServiceUnavailable`.
    fn db_pool(&self) -> Result<&sqlx::PgPool, domain::error::ApiError> {
        match &self.db {
            DbStatus::Connected(pool) => Ok(pool),
            _ => Err(domain::error::ApiError::ServiceUnavailable(
                "database not configured".into(),
            )),
        }
    }
}

/// WebSocket connection tracking state.
#[derive(Clone)]
pub struct WebSocketState {
    pub connection_permits: Arc<Semaphore>,
    pub idle_timeout: Duration,
    pub shutdown: Arc<Notify>,
    pub user_connections: Arc<RwLock<HashMap<String, usize>>>,
    pub per_user_max: usize,
}

/// Application root state.
///
/// Sub-structs (`HealthState`, `WebSocketState`) enable Axum sub-state extraction
/// so handlers only receive the fields they need.
pub struct AppState {
    pub health: Arc<HealthState>,
    pub ws: Arc<WebSocketState>,
    pub auth: Option<Arc<auth::AuthService>>,
    pub storage: Option<storage::StorageClient>,
    pub prometheus_handle: PrometheusHandle,
}

impl AppState {
    /// Convenience accessor for db_pool (delegates to HealthState).
    fn db_pool(&self) -> Result<&sqlx::PgPool, domain::error::ApiError> {
        self.health.db_pool()
    }
}

impl FromRef<AppState> for Arc<HealthState> {
    fn from_ref(app: &AppState) -> Self {
        app.health.clone()
    }
}

impl FromRef<AppState> for Arc<WebSocketState> {
    fn from_ref(app: &AppState) -> Self {
        app.ws.clone()
    }
}

pub async fn router(
    prometheus_handle: PrometheusHandle,
) -> Result<(Router, Arc<AppState>), Box<dyn std::error::Error + Send + Sync>> {
    let db = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            match PgPoolOptions::new()
                .max_connections(
                    std::env::var("DB_MAX_CONNECTIONS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(20),
                )
                .acquire_timeout(Duration::from_secs(2))
                .connect(&url)
                .await
            {
                Ok(pool) => {
                    if let Err(e) = db::run_migrations(&pool).await {
                        tracing::error!(error = %e, "database migration failed — aborting startup");
                        return Err(e.into());
                    }
                    DbStatus::Connected(pool)
                }
                Err(e) => DbStatus::ConnectionFailed(format!("connection failed: {e}")),
            }
        }
        Err(_) => DbStatus::NotConfigured,
    };

    let gauge_task = match &db {
        DbStatus::Connected(pool) => Some(metrics::spawn_pool_gauge_task(pool.clone())),
        _ => None,
    };

    let redis = match cache::RedisClient::from_env().await {
        Ok(client) => Some(Arc::new(client)),
        Err(cache::CacheError::NotConfigured) => {
            tracing::info!("REDIS_URL not set — Redis features disabled");
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "Redis connection failed — Redis features disabled");
            None
        }
    };

    let auth = auth::AuthService::from_env().map(Arc::new);
    if auth.is_none() {
        tracing::info!("JWT_SECRET not set or is CHANGE_ME — auth disabled");
    }

    let storage = storage::StorageClient::from_env();
    if let Some(ref s) = storage {
        if let Err(e) = s.ensure_bucket().await {
            tracing::warn!(error = %e, "storage bucket creation failed");
        }
    } else {
        tracing::info!("RUSTFS_ENDPOINT not set — storage disabled");
    }

    let ws_max_connections: usize = parse_env_or("WS_MAX_CONNECTIONS", 100);
    let ws_idle_timeout_secs: u64 = parse_env_or("WS_IDLE_TIMEOUT_SECS", 300);

    let ws_shutdown = Arc::new(Notify::new());
    let ws_per_user_max: usize = parse_env_or("WS_PER_USER_MAX", 10);

    let state = Arc::new(AppState {
        health: Arc::new(HealthState {
            db,
            redis,
            sidecar: PythonSidecar::from_env(),
            gauge_task,
            feature_flags: Some(domain::FeatureFlags::from_env()),
        }),
        ws: Arc::new(WebSocketState {
            connection_permits: Arc::new(Semaphore::new(ws_max_connections)),
            idle_timeout: Duration::from_secs(ws_idle_timeout_secs),
            shutdown: ws_shutdown,
            user_connections: Arc::new(RwLock::new(HashMap::new())),
            per_user_max: ws_per_user_max,
        }),
        auth,
        storage,
        prometheus_handle,
    });

    Ok((build_router(state.clone()), state))
}

pub fn router_with_state(state: AppState) -> Router {
    build_router(Arc::new(state))
}

fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new()
        .merge(health_routes())
        .route("/metrics", get(metrics_handler))
        .route("/metrics/python", get(metrics_python_proxy))
        .route("/live", get(live::ws_handler))
        .layer(middleware::from_fn(metrics::track_metrics))
        .with_state(state.clone());

    if let Some(flags) = state.health.feature_flags {
        router = router
            .layer(middleware::from_fn(maintenance_middleware))
            .layer(Extension(flags));
    }

    if let Some(auth_router) = auth_routes(&state) {
        router = router.nest("/auth", auth_router);
    }

    if let Some(storage_router) = storage_routes(&state) {
        router = router.nest("/storage", storage_router);
    }

    let db_connected = matches!(state.health.db, DbStatus::Connected(_));
    let dev_user_id = std::env::var("DEV_USER_ID")
        .ok()
        .and_then(|v| if v.is_empty() { None } else { uuid::Uuid::parse_str(&v).ok().map(|_| v) });
    if let Some(ref uid) = dev_user_id {
        tracing::info!(dev_user_id = %uid, "DEV_USER_ID set — notes available without auth");
    }

    let mount_notes = state.auth.is_some() || dev_user_id.is_some();
    if mount_notes && db_connected {
        let mut notes_router = Router::new()
            .route("/", axum::routing::get(notes::list_notes))
            .route("/", axum::routing::post(notes::create_note))
            .route("/{id}", axum::routing::get(notes::get_note))
            .route("/{id}", axum::routing::put(notes::update_note))
            .route("/{id}", axum::routing::delete(notes::delete_note))
            .layer(DefaultBodyLimit::max(128 * 1024))
            .with_state(state.clone());

        if state.auth.is_none() {
            let uid = dev_user_id.clone().unwrap();
            notes_router = notes_router
                .layer(Extension(DevUserId(uid)))
                .layer(middleware::from_fn(dev_user_middleware));
        }

        router = router.nest("/notes", notes_router);
    } else {
        let fb = Router::new()
            .route("/", get(notes_fallback).post(notes_fallback))
            .route(
                "/{id}",
                get(notes_fallback)
                    .put(notes_fallback)
                    .delete(notes_fallback),
            );
        router = router.nest("/notes", fb);
    }
    if state.auth.is_none() {
        router = router.route("/auth/me", get(auth_me_disabled));
    }

    if let Some(ref auth_svc) = state.auth {
        router = router
            .layer(middleware::from_fn(auth::middleware::auth_middleware))
            .layer(Extension(auth_svc.clone()));
        if let Some(ref redis) = state.health.redis {
            router = router.layer(Extension(redis.clone()));
        }
    }

    router
}

fn health_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/health/db", get(health_db))
        .route("/health/redis", get(health_redis))
        .route("/health/storage", get(health_storage))
        .route("/health/python", get(health_python))
        .route("/health/auth", get(health_auth))
}

fn auth_routes(state: &Arc<AppState>) -> Option<Router> {
    let (auth_svc, redis) = (&state.auth, &state.health.redis);
    let pool = match &state.health.db {
        DbStatus::Connected(p) => p,
        _ => return None,
    };
    let auth_svc = auth_svc.as_ref()?;
    let redis = redis.as_ref()?;

    let auth_state = auth::routes::AuthState {
        auth: auth_svc.clone(),
        db: pool.clone(),
        redis: redis.clone(),
        oauth: Arc::new(auth::oauth::OAuthService::new(
            auth_svc.config.google_client_id.clone(),
            auth_svc.config.google_client_secret.clone(),
            auth_svc.config.github_client_id.clone(),
            auth_svc.config.github_client_secret.clone(),
            reqwest::Client::new(),
        )),
    };
    Some(
        Router::new()
            .route("/register", axum::routing::post(auth::routes::register))
            .route("/login", axum::routing::post(auth::routes::login))
            .route("/logout", axum::routing::post(auth::routes::logout))
            .route("/forgot-password", axum::routing::post(auth::routes::forgot_password))
            .route("/reset-password", axum::routing::post(auth::routes::reset_password))
            .route("/refresh", axum::routing::post(auth::routes::refresh))
            .route("/providers", axum::routing::get(auth::routes::providers))
            .route("/me", axum::routing::get(auth::routes::me))
            .route(
                "/oauth/{provider}",
                axum::routing::get(auth::routes::oauth_redirect),
            )
            .route(
                "/oauth/{provider}/callback",
                axum::routing::get(auth::routes::oauth_callback),
            )
            .layer(middleware::from_fn(auth::metrics::track_auth_metrics))
            .with_state(auth_state),
    )
}

fn storage_routes(state: &Arc<AppState>) -> Option<Router> {
    let storage_svc = state.storage.as_ref()?;
    let storage_state = storage::routes::StorageState {
        client: reqwest::Client::new(),
        config: storage_svc.config.clone(),
    };
    Some(
        Router::new()
            .route("/{key}", axum::routing::put(storage::routes::upload))
            .route("/{key}", axum::routing::get(storage::routes::download))
            .route("/{key}", axum::routing::delete(storage::routes::delete))
            .route("/", axum::routing::get(storage::routes::list))
            .route("/presign", axum::routing::post(storage::routes::presign))
            .route(
                "/multipart/init",
                axum::routing::post(storage::routes::init_multipart),
            )
            .route(
                "/multipart/{key}/{upload_id}/part/{part_number}",
                axum::routing::put(storage::routes::upload_part),
            )
            .route(
                "/multipart/{key}/{upload_id}/complete",
                axum::routing::post(storage::routes::complete_multipart),
            )
            .route(
                "/multipart/{key}/{upload_id}",
                axum::routing::delete(storage::routes::abort_multipart),
            )
            .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
            .with_state(storage_state),
    )
}

/// Maintenance mode middleware — returns 503 when FEATURE_MAINTENANCE is enabled,
/// unless the request targets a whitelisted route (health, metrics).
pub async fn maintenance_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let path = req.uri().path();
    // Whitelist: health checks and metrics must always work
    let is_whitelisted = path == "/health"
        || path.starts_with("/health/")
        || path == "/metrics"
        || path.starts_with("/metrics/")
        || path == "/live";

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

fn no_cache() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache, no-store"),
    );
    headers
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rust = json!({
        "status": "ok",
        "service": "api",
    });

    let db = health_db_value(&state.health).await;
    let redis = health_redis_value(&state.health).await;
    let storage = health_storage_value(&state);
    let python = health_python_value(&state.health).await;
    let auth = health_auth_value(&state);

    // Truncate error details before broadcasting to WS clients (prevents
    // unbounded message growth)
    let truncate = |s: &str| -> String { s.chars().take(MAX_DETAIL_LENGTH).collect::<String>() };

    // Fire-and-forget health broadcasts to WS subscribers (must not delay HTTP response)
    // Clone values for the spawned task since they're also used in the HTTP response below
    let (db_clone, redis_clone, storage_clone, python_clone, auth_clone) = (
        db.clone(),
        redis.clone(),
        storage.clone(),
        python.clone(),
        auth.clone(),
    );
    let broadcast_state = state.clone();
    tokio::spawn(async move {
        futures_util::future::join_all([
            broadcast_event(
                &broadcast_state,
                &LiveEvent::HealthUpdate {
                    service: "rust".into(),
                    status: "ok".into(),
                    detail: None,
                },
            ),
            broadcast_event(
                &broadcast_state,
                &LiveEvent::HealthUpdate {
                    service: "db".into(),
                    status: db_clone["status"].as_str().unwrap_or("unknown").into(),
                    detail: db_clone
                        .get("error")
                        .and_then(|v| v.as_str())
                        .map(&truncate),
                },
            ),
            broadcast_event(
                &broadcast_state,
                &LiveEvent::HealthUpdate {
                    service: "redis".into(),
                    status: redis_clone["status"].as_str().unwrap_or("unknown").into(),
                    detail: redis_clone
                        .get("error")
                        .and_then(|v| v.as_str())
                        .map(&truncate),
                },
            ),
            broadcast_event(
                &broadcast_state,
                &LiveEvent::HealthUpdate {
                    service: "storage".into(),
                    status: storage_clone["status"].as_str().unwrap_or("unknown").into(),
                    detail: storage_clone
                        .get("error")
                        .and_then(|v| v.as_str())
                        .map(&truncate),
                },
            ),
            broadcast_event(
                &broadcast_state,
                &LiveEvent::HealthUpdate {
                    service: "python".into(),
                    status: python_clone["status"].as_str().unwrap_or("unknown").into(),
                    detail: python_clone
                        .get("error")
                        .and_then(|v| v.as_str())
                        .map(&truncate),
                },
            ),
            broadcast_event(
                &broadcast_state,
                &LiveEvent::HealthUpdate {
                    service: "auth".into(),
                    status: auth_clone["status"].as_str().unwrap_or("unknown").into(),
                    detail: None,
                },
            ),
        ])
        .await;
    });

    let flags = state.health.feature_flags.map(|f| {
        json!({
            "maintenance_mode": f.maintenance_mode,
        })
    });

    (
        StatusCode::OK,
        no_cache(),
        Json(json!({
            "rust": rust,
            "db": db,
            "redis": redis,
            "storage": storage,
            "python": python,
            "auth": auth,
            "feature_flags": flags,
        })),
    )
}

fn health_auth_value(state: &AppState) -> serde_json::Value {
    if state.auth.is_some() {
        json!({ "status": "ok" })
    } else {
        json!({ "status": "disabled" })
    }
}

async fn health_auth(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (StatusCode::OK, no_cache(), Json(health_auth_value(&state)))
}

async fn health_db_value(state: &HealthState) -> serde_json::Value {
    let pool = match &state.db {
        DbStatus::Connected(pool) => Some(pool),
        DbStatus::NotConfigured => {
            tracing::info!("health check: database not configured");
            return json!({ "status": "error" });
        }
        DbStatus::ConnectionFailed(msg) => {
            tracing::warn!(error = %msg, "health check: database connection failed");
            return json!({ "status": "error" });
        }
    };

    match db::health_check(pool).await {
        Ok(()) => json!({ "status": "ok" }),
        Err(e) => {
            tracing::warn!(error = %e, "health check: database unhealthy");
            json!({ "status": "error" })
        }
    }
}

async fn health_db(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let value = health_db_value(&state.health).await;
    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

async fn health_redis_value(state: &HealthState) -> serde_json::Value {
    match &state.redis {
        Some(redis) => match redis.ping().await {
            Ok(()) => json!({ "status": "ok" }),
            Err(e) => {
                tracing::warn!(error = %e, "health check: Redis ping failed");
                json!({ "status": "error" })
            }
        },
        None => {
            tracing::info!("health check: Redis not configured");
            json!({ "status": "error" })
        }
    }
}

async fn health_redis(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let value = health_redis_value(&state.health).await;
    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

fn health_storage_value(state: &AppState) -> serde_json::Value {
    match &state.storage {
        Some(_) => json!({ "status": "ok" }),
        None => {
            tracing::info!("health check: storage not configured");
            json!({ "status": "error" })
        }
    }
}

async fn health_storage(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let value = health_storage_value(&state);
    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

fn format_health_value(v: &serde_json::Value) -> serde_json::Value {
    json!({
        "status": v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown"),
        "service": v.get("service").and_then(|s| s.as_str()).unwrap_or("unknown"),
    })
}

async fn health_python_value(state: &HealthState) -> serde_json::Value {
    match state.sidecar.health().await {
        Ok(v) => format_health_value(&v),
        Err(e) => sidecar_error_json(&e, state.sidecar.socket_path()),
    }
}

fn sidecar_error_json(
    e: &py_sidecar::SidecarError,
    _socket_path: &std::path::Path,
) -> serde_json::Value {
    tracing::warn!(error = %e, "health check: Python sidecar unavailable");
    json!({ "status": "unavailable" })
}

async fn health_python(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    let trace_id = req
        .headers()
        .get("x-trace-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !trace_id.is_empty() {
        tracing::info!(%trace_id, "health check via sidecar with propagated trace_id");
    }

    let value = if trace_id.is_empty() {
        health_python_value(&state.health).await
    } else {
        match state
            .health
            .sidecar
            .get_with_trace_id("/health", trace_id, None)
            .await
        {
            Ok(v) => format_health_value(&v),
            Err(e) => sidecar_error_json(&e, state.health.sidecar.socket_path()),
        }
    };

    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let body = metrics::render_metrics(&state.prometheus_handle);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
}

async fn metrics_python_proxy(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.health.sidecar.get_raw("/metrics").await {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            body,
        ),
        Err(py_sidecar::SidecarError::HttpError { status, body }) => {
            let code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            tracing::warn!(status = %status, "Python sidecar returned HTTP error for /metrics");
            (
                code,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                format!("# Python metrics error: {body}").into_bytes(),
            )
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to proxy Python metrics");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                format!("# Python metrics unavailable: {e}").into_bytes(),
            )
        }
    }
}

/// Fallback /auth/me when auth not configured.
/// Returns 200 with `{"status":"disabled"}` so browser doesn't log 404.
async fn auth_me_disabled() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        "{\"status\":\"disabled\"}",
    )
}

/// Fallback for notes endpoints when auth or database not configured.
async fn notes_fallback() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": "notes unavailable — auth or database not configured"})),
    )
}

/// Middleware that injects a mock `AuthUser` using the `DEV_USER_ID` from
/// request extensions.  Only applied on the notes sub-router when auth is
/// disabled but `DEV_USER_ID` is set.
async fn dev_user_middleware(
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let dev_user_id = req.extensions().get::<DevUserId>().map(|d| d.0.clone());
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

pub mod test_helpers;

#[cfg(test)]
mod proptests;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            health: Arc::new(HealthState {
                db: DbStatus::NotConfigured,
                redis: None,
                sidecar: PythonSidecar::new(
                    "/tmp/__nonexistent_test_socket__.sock",
                    Duration::from_secs(1),
                    0,
                ),
                gauge_task: None,
                feature_flags: Some(domain::FeatureFlags {
                    maintenance_mode: false,
                }),
            }),
            ws: Arc::new(WebSocketState {
                connection_permits: Arc::new(Semaphore::new(100)),
                idle_timeout: Duration::from_secs(300),
                shutdown: Arc::new(Notify::new()),
                user_connections: Arc::new(RwLock::new(HashMap::new())),
                per_user_max: 10,
            }),
            auth: None,
            storage: None,
            prometheus_handle: metrics::init_metrics_recorder(),
        }
    }

    #[tokio::test]
    async fn health_returns_200() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_db_returns_503_when_not_configured() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/db")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn health_redis_returns_503_when_not_configured() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/redis")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn health_storage_returns_503_when_not_configured() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/storage")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn health_python_returns_503_when_no_socket() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/python")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_prometheus_text() {
        let app = router_with_state(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn normalize_route_bounds_known_paths() {
        assert_eq!(metrics::normalize_route("/health"), "/health");
        assert_eq!(metrics::normalize_route("/health/db"), "/health/db");
        assert_eq!(metrics::normalize_route("/health/redis"), "/health/redis");
        assert_eq!(
            metrics::normalize_route("/health/storage"),
            "/health/storage"
        );
    }
}
