use crate::{AppState, HealthState};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::{Json, extract::State, http::Request};
use serde_json::json;
use std::sync::OnceLock;
use std::time::Instant;

use crate::live::{LiveEvent, broadcast_event};

/// Max length for health error details broadcast to WS clients.
const MAX_DETAIL_LENGTH: usize = 500;

/// How long to cache health check results before re-checking.
const HEALTH_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(1);

static NO_CACHE_HEADERS: OnceLock<HeaderMap> = OnceLock::new();

fn no_cache() -> HeaderMap {
    NO_CACHE_HEADERS
        .get_or_init(|| {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static("no-cache, no-store"),
            );
            headers
        })
        .clone()
}

pub(crate) async fn health(State(state): State<std::sync::Arc<AppState>>) -> impl IntoResponse {
    let rust = json!({
        "status": "ok",
        "service": "api",
    });

    let (db, redis, python) = tokio::join!(
        health_db_value(&state.health),
        health_redis_value(&state.health),
        health_python_value(&state.health),
    );
    let storage = health_storage_value(&state);
    let auth = health_auth_value(&state);

    let truncate = |s: &str| -> String { s.chars().take(MAX_DETAIL_LENGTH).collect::<String>() };

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

    let flags = json!({
        "maintenance_mode": state.health.feature_flags.maintenance_mode,
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

pub(crate) fn health_auth_value(state: &AppState) -> serde_json::Value {
    if state.auth.is_some() {
        json!({ "status": "ok" })
    } else {
        json!({ "status": "disabled" })
    }
}

pub(crate) async fn health_auth(
    State(state): State<std::sync::Arc<AppState>>,
) -> impl IntoResponse {
    (StatusCode::OK, no_cache(), Json(health_auth_value(&state)))
}

pub(crate) async fn health_db_value(state: &HealthState) -> serde_json::Value {
    // Fast path: cache hit (read lock, no contention with other readers)
    if let Some((cached_at, cached_val)) = state.db_health_cache.read().await.as_ref()
        && cached_at.elapsed() < HEALTH_CACHE_TTL
    {
        return cached_val.clone();
    }
    // Cache miss or expired: acquire write lock and double-check
    let mut cache = state.db_health_cache.write().await;
    if let Some((cached_at, cached_val)) = cache.as_ref()
        && cached_at.elapsed() < HEALTH_CACHE_TTL
    {
        return cached_val.clone();
    }
    let value = match &state.db {
        crate::DbStatus::Connected(pool) => match db::health_check(Some(pool)).await {
            Ok(()) => json!({ "status": "ok" }),
            Err(e) => {
                tracing::warn!(error = %e, "health check: database unhealthy");
                json!({ "status": "error" })
            }
        },
        crate::DbStatus::NotConfigured => {
            tracing::info!("health check: database not configured");
            json!({ "status": "disabled" })
        }
        crate::DbStatus::ConnectionFailed(msg) => {
            tracing::warn!(error = %msg, "health check: database connection failed");
            json!({ "status": "error" })
        }
    };
    // Only cache successful checks — don't amplify transient failures
    if value["status"] == "ok" {
        *cache = Some((Instant::now(), value.clone()));
    }
    value
}

pub(crate) async fn health_db(State(state): State<std::sync::Arc<AppState>>) -> impl IntoResponse {
    let value = health_db_value(&state.health).await;
    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

pub(crate) async fn health_redis_value(state: &HealthState) -> serde_json::Value {
    if let Some((cached_at, cached_val)) = state.redis_health_cache.read().await.as_ref()
        && cached_at.elapsed() < HEALTH_CACHE_TTL
    {
        return cached_val.clone();
    }
    let mut cache = state.redis_health_cache.write().await;
    if let Some((cached_at, cached_val)) = cache.as_ref()
        && cached_at.elapsed() < HEALTH_CACHE_TTL
    {
        return cached_val.clone();
    }
    let value = match &state.redis {
        Some(redis) => match redis.ping().await {
            Ok(()) => json!({ "status": "ok" }),
            Err(e) => {
                tracing::warn!(error = %e, "health check: Redis ping failed");
                json!({ "status": "error" })
            }
        },
        None => {
            tracing::info!("health check: Redis not configured");
            json!({ "status": "disabled" })
        }
    };
    if value["status"] == "ok" {
        *cache = Some((Instant::now(), value.clone()));
    }
    value
}

pub(crate) async fn health_redis(
    State(state): State<std::sync::Arc<AppState>>,
) -> impl IntoResponse {
    let value = health_redis_value(&state.health).await;
    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

pub(crate) fn health_storage_value(state: &AppState) -> serde_json::Value {
    match &state.storage {
        Some(_) => json!({ "status": "ok" }),
        None => {
            tracing::info!("health check: storage not configured");
            json!({ "status": "disabled" })
        }
    }
}

pub(crate) async fn health_storage(
    State(state): State<std::sync::Arc<AppState>>,
) -> impl IntoResponse {
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

pub(crate) async fn health_python_value(state: &HealthState) -> serde_json::Value {
    if let Some((cached_at, cached_val)) = state.py_health_cache.read().await.as_ref()
        && cached_at.elapsed() < HEALTH_CACHE_TTL
    {
        return cached_val.clone();
    }
    let mut cache = state.py_health_cache.write().await;
    if let Some((cached_at, cached_val)) = cache.as_ref()
        && cached_at.elapsed() < HEALTH_CACHE_TTL
    {
        return cached_val.clone();
    }
    let value = match state.sidecar.health().await {
        Ok(v) => format_health_value(&v),
        Err(e) => sidecar_error_json(&e),
    };
    if value["status"] == "ok" {
        *cache = Some((Instant::now(), value.clone()));
    }
    value
}

fn sidecar_error_json(e: &py_sidecar::SidecarError) -> serde_json::Value {
    tracing::warn!(error = %e, "health check: Python sidecar unavailable");
    json!({ "status": "unavailable" })
}

pub(crate) async fn health_python(
    State(state): State<std::sync::Arc<AppState>>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    let trace_id = req
        .headers()
        .get("x-trace-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let value = if trace_id.is_empty() {
        health_python_value(&state.health).await
    } else {
        tracing::info!(%trace_id, "health check via sidecar with propagated trace_id");
        match state
            .health
            .sidecar
            .get_with_trace_id("/health", trace_id, None)
            .await
        {
            Ok(v) => format_health_value(&v),
            Err(e) => sidecar_error_json(&e),
        }
    };

    let status = if value["status"] == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, no_cache(), Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use crate::{DbStatus, HealthState};

    fn test_sidecar() -> py_sidecar::PythonSidecar {
        py_sidecar::PythonSidecar::new(
            "/tmp/__nonexistent_test_socket__.sock",
            Duration::from_secs(1),
            0,
        )
    }

    fn health_state_with(
        db: DbStatus,
        py_cache: Option<(Instant, serde_json::Value)>,
        db_cache: Option<(Instant, serde_json::Value)>,
        redis_cache: Option<(Instant, serde_json::Value)>,
    ) -> HealthState {
        HealthState {
            db,
            redis: None,
            sidecar: test_sidecar(),
            gauge_task: None,
            feature_flags: domain::FeatureFlags {
                maintenance_mode: false,
            },
            py_health_cache: Arc::new(tokio::sync::RwLock::new(py_cache)),
            db_health_cache: Arc::new(tokio::sync::RwLock::new(db_cache)),
            redis_health_cache: Arc::new(tokio::sync::RwLock::new(redis_cache)),
        }
    }

    #[tokio::test]
    async fn test_db_cache_hit_returns_cached_value() {
        let state = health_state_with(
            DbStatus::NotConfigured,
            None,
            Some((Instant::now(), json!({"status": "ok"}))),
            None,
        );
        let result = health_db_value(&state).await;
        assert_eq!(result["status"], "ok");
    }

    #[tokio::test]
    async fn test_db_cache_expired_falls_through() {
        let state = health_state_with(
            DbStatus::NotConfigured,
            None,
            Some((
                Instant::now() - HEALTH_CACHE_TTL - Duration::from_secs(1),
                json!({"status": "ok"}),
            )),
            None,
        );
        let result = health_db_value(&state).await;
        assert_eq!(result["status"], "disabled");
    }

    #[tokio::test]
    async fn test_db_cache_only_caches_ok() {
        let state = health_state_with(DbStatus::NotConfigured, None, None, None);
        let result = health_db_value(&state).await;
        assert_eq!(result["status"], "disabled");
        let cache = state.db_health_cache.read().await;
        assert!(cache.is_none(), "should not cache 'disabled' status");
    }

    #[tokio::test]
    async fn test_python_cache_hit_returns_cached_value() {
        let state = health_state_with(
            DbStatus::NotConfigured,
            Some((Instant::now(), json!({"status": "ok"}))),
            None,
            None,
        );
        let result = health_python_value(&state).await;
        assert_eq!(result["status"], "ok");
    }

    #[tokio::test]
    async fn test_python_cache_expired_falls_through() {
        let state = health_state_with(
            DbStatus::NotConfigured,
            Some((
                Instant::now() - HEALTH_CACHE_TTL - Duration::from_secs(1),
                json!({"status": "ok"}),
            )),
            None,
            None,
        );
        let result = health_python_value(&state).await;
        assert_eq!(result["status"], "unavailable");
    }

    #[tokio::test]
    async fn test_redis_cache_hit_returns_cached_value() {
        let state = health_state_with(
            DbStatus::NotConfigured,
            None,
            None,
            Some((Instant::now(), json!({"status": "ok"}))),
        );
        let result = health_redis_value(&state).await;
        assert_eq!(result["status"], "ok");
    }

    #[tokio::test]
    async fn test_redis_cache_expired_falls_through() {
        let state = health_state_with(
            DbStatus::NotConfigured,
            None,
            None,
            Some((
                Instant::now() - HEALTH_CACHE_TTL - Duration::from_secs(1),
                json!({"status": "ok"}),
            )),
        );
        let result = health_redis_value(&state).await;
        assert_eq!(result["status"], "disabled");
    }
}
