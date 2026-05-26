use axum::{
    Router,
    extract::{DefaultBodyLimit, Extension, FromRef},
    routing::get,
};
use metrics_exporter_prometheus::PrometheusHandle;
use py_sidecar::PythonSidecar;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

pub mod fallback;
pub mod health;
pub mod live;
pub mod metrics;
pub mod middleware;
pub mod notes;

/// Parse an env var, logging a warning on parse failure before returning default.
pub(crate) fn parse_env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
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
pub(crate) fn db_err(e: sqlx::Error, context: &'static str) -> domain::error::ApiError {
    tracing::warn!(error = %e, "{context}");
    ::metrics::counter!("notes_query_errors_total").increment(1);
    domain::error::ApiError::InternalError(context.into())
}

/// Newtype to carry DEV_USER_ID via request extensions.
#[derive(Clone)]
pub(crate) struct DevUserId(String);

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
    pub feature_flags: domain::FeatureFlags,
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
    pub allowed_origin: Option<String>,
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
    let allowed_origin: Option<String> = std::env::var("ALLOWED_ORIGIN")
        .ok()
        .filter(|v| !v.is_empty());

    let state = Arc::new(AppState {
        health: Arc::new(HealthState {
            db,
            redis,
            sidecar: PythonSidecar::from_env(),
            gauge_task,
            feature_flags: domain::FeatureFlags::from_env(),
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
        allowed_origin,
    });

    Ok((build_router(state.clone()), state))
}

pub fn router_with_state(state: AppState) -> Router {
    build_router(Arc::new(state))
}

fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new()
        .merge(health_routes())
        .route("/metrics", get(metrics::metrics_handler))
        .route("/metrics/python", get(metrics::metrics_python_proxy))
        .route("/live", get(live::ws_handler))
        .layer(axum::middleware::from_fn(metrics::track_metrics))
        .with_state(state.clone());

    let flags = state.health.feature_flags;
    router = router
        .layer(axum::middleware::from_fn(
            middleware::maintenance_middleware,
        ))
        .layer(Extension(flags));

    if let Some(auth_router) = auth_routes(&state) {
        router = router.nest("/auth", auth_router);
    }

    if let Some(storage_router) = storage_routes(&state) {
        router = router.nest("/storage", storage_router);
    }

    let db_connected = matches!(state.health.db, DbStatus::Connected(_));
    let dev_user_id = std::env::var("DEV_USER_ID").ok().and_then(|v| {
        if v.is_empty() {
            None
        } else {
            uuid::Uuid::parse_str(&v).ok().map(|_| v)
        }
    });
    if let Some(ref uid) = dev_user_id {
        if std::env::var("PRODUCTION").is_ok() {
            tracing::warn!(dev_user_id = %uid, "DEV_USER_ID is set in PRODUCTION mode — notes bypass authentication");
        } else {
            tracing::info!(dev_user_id = %uid, "DEV_USER_ID set — notes available without auth");
        }
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
                .layer(axum::middleware::from_fn(middleware::dev_user_middleware));
        }

        router = router.nest("/notes", notes_router);
    } else {
        let fb = Router::new()
            .route(
                "/",
                get(fallback::notes_fallback).post(fallback::notes_fallback),
            )
            .route(
                "/{id}",
                get(fallback::notes_fallback)
                    .put(fallback::notes_fallback)
                    .delete(fallback::notes_fallback),
            );
        router = router.nest("/notes", fb);
    }
    if state.auth.is_none() {
        router = router.route("/auth/me", get(fallback::auth_me_disabled));
    }

    if let Some(ref auth_svc) = state.auth {
        router = router
            .layer(axum::middleware::from_fn(auth::middleware::auth_middleware))
            .layer(Extension(auth_svc.clone()));
        if let Some(ref redis) = state.health.redis {
            router = router.layer(Extension(redis.clone()));
        }
    }

    router
}

fn health_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health))
        .route("/health/db", get(health::health_db))
        .route("/health/redis", get(health::health_redis))
        .route("/health/storage", get(health::health_storage))
        .route("/health/python", get(health::health_python))
        .route("/health/auth", get(health::health_auth))
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
            .route(
                "/forgot-password",
                axum::routing::post(auth::routes::forgot_password),
            )
            .route(
                "/reset-password",
                axum::routing::post(auth::routes::reset_password),
            )
            .route("/refresh", axum::routing::post(auth::routes::refresh))
            .route("/providers", axum::routing::get(auth::routes::providers))
            .route(
                "/me",
                axum::routing::get(auth::routes::me).delete(auth::routes::delete_account),
            )
            .route(
                "/oauth/{provider}",
                axum::routing::get(auth::routes::oauth_redirect),
            )
            .route(
                "/oauth/{provider}/callback",
                axum::routing::get(auth::routes::oauth_callback),
            )
            .layer(axum::middleware::from_fn(auth::metrics::track_auth_metrics))
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
                feature_flags: domain::FeatureFlags {
                    maintenance_mode: false,
                },
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
            allowed_origin: None,
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
