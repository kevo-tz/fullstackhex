use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Extension, State},
    http::Request,
    middleware,
    routing::get,
};
use metrics_exporter_prometheus::PrometheusHandle;
use py_sidecar::PythonSidecar;
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

pub mod metrics;

/// Status of the database connection pool.
pub enum DbStatus {
    NotConfigured,
    Connected(PgPool),
    ConnectionFailed(String),
}

pub struct AppState {
    pub db: DbStatus,
    pub redis: Option<Arc<cache::RedisClient>>,
    pub auth: Option<Arc<auth::AuthService>>,
    pub storage: Option<storage::StorageClient>,
    pub sidecar: PythonSidecar,
    pub prometheus_handle: PrometheusHandle,
    pub gauge_task: Option<tokio::task::AbortHandle>,
}

pub async fn router(
    prometheus_handle: PrometheusHandle,
) -> Result<(Router, Arc<AppState>), Box<dyn std::error::Error + Send + Sync>> {
    let db = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            match PgPoolOptions::new()
                .max_connections(5)
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

    let state = Arc::new(AppState {
        db,
        redis,
        auth,
        storage,
        sidecar: PythonSidecar::from_env(),
        prometheus_handle,
        gauge_task,
    });

    Ok((build_router(state.clone()), state))
}

pub fn router_with_state(state: AppState) -> Router {
    build_router(Arc::new(state))
}

fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/health/db", get(health_db))
        .route("/health/redis", get(health_redis))
        .route("/health/storage", get(health_storage))
        .route("/health/python", get(health_python))
        .route("/health/auth", get(health_auth))
        .route("/metrics", get(metrics_handler))
        .route("/metrics/python", get(metrics_python_proxy))
        .layer(middleware::from_fn(metrics::track_metrics))
        .with_state(state.clone());

    // Nest auth routes with their own state
    if let (Some(auth_svc), Some(redis)) = (&state.auth, &state.redis)
        && let DbStatus::Connected(ref pool) = state.db
    {
        let auth_state = auth::routes::AuthState {
            auth: auth_svc.clone(),
            db: pool.clone(),
            redis: redis.clone(),
        };
        let auth_router = Router::new()
            .route("/register", axum::routing::post(auth::routes::register))
            .route("/login", axum::routing::post(auth::routes::login))
            .route("/logout", axum::routing::post(auth::routes::logout))
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
            .with_state(auth_state);
        router = router.nest("/auth", auth_router);
    }

    // Nest storage routes with their own state
    if let Some(ref storage_svc) = state.storage {
        let storage_state = storage::routes::StorageState {
            client: reqwest::Client::new(),
            config: storage_svc.config.clone(),
        };
        let storage_router = Router::new()
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
            .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10MB upload limit
            .with_state(storage_state);
        router = router.nest("/storage", storage_router);
    }

    // Add auth middleware globally when auth is configured
    if let Some(ref auth_svc) = state.auth {
        router = router
            .layer(middleware::from_fn(auth::middleware::auth_middleware))
            .layer(Extension(auth_svc.clone()));
        // Also inject Redis so the middleware can check JWT blacklist
        if let Some(ref redis) = state.redis {
            router = router.layer(Extension(redis.clone()));
        }
    }

    router
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
        "version": env!("CARGO_PKG_VERSION")
    });

    let db = health_db_value(&state).await;
    let redis = health_redis_value(&state).await;
    let storage = health_storage_value(&state);
    let python = health_python_value(&state).await;
    let auth = health_auth_value(&state);

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
        })),
    )
}

fn health_auth_value(state: &AppState) -> serde_json::Value {
    if state.auth.is_some() {
        json!({ "status": "ok" })
    } else {
        json!({
            "status": "disabled",
            "fix": "JWT_SECRET not set or is CHANGE_ME — auth disabled. Set a secure JWT_SECRET in .env and restart."
        })
    }
}

async fn health_auth(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (StatusCode::OK, no_cache(), Json(health_auth_value(&state)))
}

async fn health_db_value(state: &AppState) -> serde_json::Value {
    let pool = match &state.db {
        DbStatus::Connected(pool) => Some(pool),
        DbStatus::NotConfigured => None,
        DbStatus::ConnectionFailed(msg) => {
            return json!({
                "status": "error",
                "error": msg,
                "fix": "Check that PostgreSQL is running and DATABASE_URL is correct in .env. Then restart the backend."
            });
        }
    };

    match db::health_check(pool).await {
        Ok(()) => json!({ "status": "ok" }),
        Err(e) => {
            let (error, fix) = match &e {
                db::DbError::NotConfigured => (
                    "database not configured",
                    "Set DATABASE_URL in .env and restart the backend.",
                ),
                db::DbError::PoolTimeout(_) => (
                    "database pool timeout",
                    "The database pool is exhausted. Check PostgreSQL connection and increase DB_MAX_CONNECTIONS if needed.",
                ),
                db::DbError::QueryFailed(_) => (
                    "database query failed",
                    "Check that PostgreSQL is running and the database exists.",
                ),
                db::DbError::MigrationFailed(_) => (
                    "database migration failed",
                    "Check the migration files in backend/db/migrations/ and run: make migrate",
                ),
            };
            json!({ "status": "error", "error": error, "fix": fix })
        }
    }
}

async fn health_db(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let value = health_db_value(&state).await;
    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

async fn health_redis_value(state: &AppState) -> serde_json::Value {
    match &state.redis {
        Some(redis) => match redis.ping().await {
            Ok(()) => json!({ "status": "ok" }),
            Err(e) => json!({ "status": "error", "error": e.to_string() }),
        },
        None => json!({
            "status": "error",
            "error": "Redis not configured",
            "fix": "Set REDIS_URL in .env and restart the backend."
        }),
    }
}

async fn health_redis(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let value = health_redis_value(&state).await;
    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

fn health_storage_value(state: &AppState) -> serde_json::Value {
    match &state.storage {
        Some(s) => json!({ "status": "ok", "bucket": s.config.bucket }),
        None => json!({
            "status": "error",
            "error": "Storage not configured",
            "fix": "Set RUSTFS_ENDPOINT, RUSTFS_ACCESS_KEY, and RUSTFS_SECRET_KEY in .env."
        }),
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

async fn health_python_value(state: &AppState) -> serde_json::Value {
    match state.sidecar.health().await {
        Ok(v) => json!({
            "status": v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown"),
            "service": v.get("service").and_then(|s| s.as_str()).unwrap_or("unknown"),
            "version": v.get("version").and_then(|s| s.as_str()).unwrap_or("unknown"),
        }),
        Err(e) => {
            let sock_display = state.sidecar.socket_path().display();
            let (error_msg, fix_msg) = match &e {
                py_sidecar::SidecarError::SocketNotFound(_) => (
                    "socket not found".to_string(),
                    format!("Start the Python sidecar: make dev starts it automatically, or run: cd py-api && uv run uvicorn app.main:app --uds {sock_display}"),
                ),
                py_sidecar::SidecarError::ConnectionFailed(msg) => (
                    format!("connection failed: {msg}"),
                    format!("Check that the Python sidecar is running. Run: cd py-api && uv run uvicorn app.main:app --uds {sock_display}"),
                ),
                py_sidecar::SidecarError::Timeout(d) => (
                    format!("request timed out after {d:?}"),
                    format!("The Python sidecar is not responding. Restart it with: cd py-api && uv run uvicorn app.main:app --uds {sock_display}"),
                ),
                py_sidecar::SidecarError::InvalidInput(msg) => (
                    format!("invalid input: {msg}"),
                    "The request contains invalid characters.".to_string(),
                ),
                py_sidecar::SidecarError::InvalidResponse(msg) => (
                    format!("invalid response: {msg}"),
                    "The Python sidecar returned an unexpected response. Check its logs for errors.".to_string(),
                ),
                py_sidecar::SidecarError::HttpError { status, body } => (
                    format!("HTTP {status}: {body}"),
                    "The Python sidecar returned an HTTP error. Check its logs for details.".to_string(),
                ),
            };
            json!({ "status": "unavailable", "error": error_msg, "fix": fix_msg })
        }
    }
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
        health_python_value(&state).await
    } else {
        match state
            .sidecar
            .get_with_trace_id("/health", trace_id, None)
            .await
        {
            Ok(v) => json!({
                "status": v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown"),
                "service": v.get("service").and_then(|s| s.as_str()).unwrap_or("unknown"),
                "version": v.get("version").and_then(|s| s.as_str()).unwrap_or("unknown"),
            }),
            Err(e) => {
                let sock_display = state.sidecar.socket_path().display();
                let (error_msg, fix_msg) = match &e {
                    py_sidecar::SidecarError::SocketNotFound(_) => (
                        "socket not found".to_string(),
                        format!("Start the Python sidecar: make dev starts it automatically, or run: cd py-api && uv run uvicorn app.main:app --uds {sock_display}"),
                    ),
                    py_sidecar::SidecarError::ConnectionFailed(msg) => (
                        format!("connection failed: {msg}"),
                        format!("Check that the Python sidecar is running. Run: cd py-api && uv run uvicorn app.main:app --uds {sock_display}"),
                    ),
                    py_sidecar::SidecarError::Timeout(d) => (
                        format!("request timed out after {d:?}"),
                        format!("The Python sidecar is not responding. Restart it with: cd py-api && uv run uvicorn app.main:app --uds {sock_display}"),
                    ),
                    py_sidecar::SidecarError::InvalidInput(msg) => (
                        format!("invalid input: {msg}"),
                        "The request contains invalid characters.".to_string(),
                    ),
                    py_sidecar::SidecarError::InvalidResponse(msg) => (
                        format!("invalid response: {msg}"),
                        "The Python sidecar returned an unexpected response. Check its logs for errors.".to_string(),
                    ),
                    py_sidecar::SidecarError::HttpError { status, body } => (
                        format!("HTTP {status}: {body}"),
                        "The Python sidecar returned an HTTP error. Check its logs for details.".to_string(),
                    ),
                };
                json!({ "status": "unavailable", "error": error_msg, "fix": fix_msg })
            }
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
    match state.sidecar.get_raw("/metrics").await {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            db: DbStatus::NotConfigured,
            redis: None,
            auth: None,
            storage: None,
            sidecar: PythonSidecar::new(
                "/tmp/__nonexistent_test_socket__.sock",
                Duration::from_secs(1),
                0,
            ),
            prometheus_handle: metrics::init_metrics_recorder(),
            gauge_task: None,
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
