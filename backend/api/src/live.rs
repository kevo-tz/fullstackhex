//! WebSocket bridge for live events.
//!
//! Listens for health updates via Redis pub/sub and forwards them to
//! connected browser clients over WebSocket. Falls back to HTTP polling
//! when Redis is unavailable.
//!
//! ```text
//! CONNECTED CLIENT          WS HANDLER           REDIS PUB/SUB
//! ───────────────    ───────────────────    ───────────────────
//!      │                     │                       │
//!      │── HTTP Upgrade ──▶  │                       │
//!      │                     │── subscribe ────────▶ │
//!      │                     │◀─ mpsc:Receiver ─────│
//!      │◀─ 101 Switching ───│                       │
//!      │                     │                       │
//!      │◀─ WS Message ─────│◀─ forward event ──────│ (Redis PUBLISH)
//!      │     (JSON event)   │                       │
//! ```

use crate::AppState;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use cache::pubsub::PubSubMessage;
use serde::{Deserialize, Serialize};

use futures_util::sink::SinkExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::Notify;

/// Redis pub/sub channel for live events.
const LIVE_EVENTS_CHANNEL: &str = "live:events";

/// Global active connection counter for metrics.
static ACTIVE_WS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

/// Events that can be broadcast over the live channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum LiveEvent {
    #[serde(rename = "health_update")]
    HealthUpdate {
        service: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    #[serde(rename = "auth_event")]
    AuthEvent {
        kind: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
    #[serde(rename = "connection_status")]
    ConnectionStatus {
        status: String, // "connected", "reconnecting", "offline"
    },
}

/// Publish an event to the live Redis channel.
///
/// Returns early if Redis is not configured — no error, just silent skip.
/// Events are serialized to JSON and published to the `live:events` Redis channel
/// through the cache crate's pub/sub methods (not raw fred) for correct namespacing.
pub async fn broadcast_event(state: &AppState, event: &LiveEvent) {
    let redis = match &state.redis {
        Some(r) => r,
        None => return, // Redis not configured — event is dropped (polling fallback handles this)
    };

    let payload = match serde_json::to_string(event) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!(error = %e, "failed to serialize LiveEvent");
            return;
        }
    };

    if let Err(e) = redis.publish(LIVE_EVENTS_CHANNEL, &payload).await {
        tracing::warn!(error = %e, "failed to publish live event");
        return;
    }

    ::metrics::counter!("ws_events_published_total").increment(1);
}

/// Handle WebSocket upgrade requests.
///
/// Returns 404 Not Found when Redis is disabled — the frontend falls back
/// to HTTP polling automatically. When auth is configured (`state.auth` is
/// `Some`), the endpoint requires either a valid `?token=<jwt>` query param
/// or a valid session cookie. Returns 401 Unauthorized when auth is
/// configured but no valid credentials are provided.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Auth check: when auth is configured, require valid credentials
    let maybe_user_id = if let Some(ref auth_service) = state.auth {
        match (&state.redis, token_from_query(&uri)) {
            // Token from query param: validate + blacklist check + extract user_id
            (Some(redis), Some(token)) => {
                match auth_service.jwt.validate_token(&token) {
                    Ok(claims) => {
                        if is_jti_blacklisted(redis, &claims.jti).await {
                            tracing::info!(jti = %claims.jti, "WS connection rejected — blacklisted JWT");
                            None
                        } else {
                            Some(claims.sub)
                        }
                    }
                    Err(_) => None,
                }
            }
            // Try cookie auth
            _ => cookie_authenticated(&headers, auth_service, &state).await,
        }
    } else {
        None
    };

    if state.auth.is_some() && maybe_user_id.is_none() {
        tracing::info!("WS connection rejected — not authenticated");
        return (
            StatusCode::UNAUTHORIZED,
            "{\"error\":\"Authentication required\"}",
        )
            .into_response();
    }

    // Validate Origin header against ALLOWED_ORIGIN when configured
    if let Some(allowed) = std::env::var("ALLOWED_ORIGIN")
        .ok()
        .filter(|s| !s.is_empty())
    {
        let origin = headers
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !origin.is_empty() {
            let allowed_host = allowed
                .trim_start_matches("https://")
                .trim_start_matches("http://");
            // Extract host from Origin URI for exact matching (prevents
            // substring bypass like "evil-example.com" when "example.com" is allowed)
            let origin_host = origin
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .split('/')
                .next()
                .unwrap_or("");
            let origin_host = origin_host.split(':').next().unwrap_or("");
            let matches = origin_host == allowed_host
                || origin_host.strip_prefix(".").map_or(false, |s| {
                    format!(".{s}").as_str() == allowed_host
                        || allowed_host == s
                });
            if !matches {
                tracing::warn!(%origin, "WS connection rejected — Origin not allowed");
                return (
                    StatusCode::UPGRADE_REQUIRED,
                    "{\"error\":\"Origin not allowed\"}",
                )
                    .into_response();
            }
        }
    }

    // Per-user quota check — read-only, increment after all other guards pass
    let user_id = maybe_user_id.clone();
    if let Some(ref uid) = maybe_user_id {
        let conns = state.ws_user_connections.lock().await;
        if *conns.get(uid).unwrap_or(&0) >= state.ws_per_user_max {
            tracing::warn!(user_id = %uid, "WS connection rejected — per-user limit reached");
            return (
                StatusCode::TOO_MANY_REQUESTS,
                "{\"error\":\"Too many connections from this user\"}",
            )
                .into_response();
        }
    }

    let redis = match &state.redis {
        Some(r) => r.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                "{\"error\":\"WebSocket not available — Redis is disabled\"}",
            )
                .into_response();
        }
    };

    // Acquire semaphore permit (non-blocking fail returns 503)
    let permits = state.ws_connection_permits.clone();
    let permit = match permits.try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            tracing::warn!("WS connection limit reached — rejecting connection");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "{\"error\":\"Server at capacity — too many WebSocket connections\"}",
            )
                .into_response();
        }
    };

    // All guards passed — now increment per-user count (paired with WsUserGuard decrement)
    if let Some(ref uid) = user_id {
        let mut conns = state.ws_user_connections.lock().await;
        *conns.entry(uid.clone()).or_insert(0) += 1;
    }

    let idle_timeout = state.ws_idle_timeout;
    let ws_shutdown = state.ws_shutdown.clone();
    let user_connections = state.ws_user_connections.clone();
    ws.on_upgrade(move |socket| {
        handle_socket(socket, redis, idle_timeout, ws_shutdown, user_connections, user_id, permit)
    })
}

/// Extract JWT token from WebSocket upgrade URI query parameter.
///
/// Browser WebSocket API does not support custom headers, so the token
/// is passed as a query parameter: `wss://host/api/live?token=<jwt>`.
/// JWT tokens are base64url-encoded (alphanumeric + `.` + `-` + `_`) —
/// no percent-decoding is needed for standard tokens.
fn token_from_query(uri: &Uri) -> Option<String> {
    let query = uri.query()?;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("token=") {
            return Some(value.to_string());
        }
    }
    None
}

/// Authenticate a WebSocket connection via session cookie.
///
/// Extracts the `session=` cookie from the `Cookie` header, looks up the
/// session token in Redis, and validates it with the JWT service.
/// Returns the user_id on success.
async fn cookie_authenticated(
    headers: &HeaderMap,
    auth_service: &auth::AuthService,
    state: &AppState,
) -> Option<String> {
    let session_id = match headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix("session=")
            })
        }) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return None,
    };

    let redis = match &state.redis {
        Some(r) => r.clone(),
        None => return None,
    };

    let token: Option<String> = redis.cache_get("session", &session_id).await.unwrap_or(None);
    match token {
        Some(t) => match auth_service.jwt.validate_token(&t) {
            Ok(claims) => {
                if is_jti_blacklisted(&redis, &claims.jti).await {
                    None
                } else {
                    Some(claims.sub)
                }
            }
            Err(_) => None,
        },
        None => None,
    }
}

/// Check Redis blacklist for a JWT identifier.
/// Returns `false` (allow) on cache-miss. On Redis error, rejects (fail-closed).
async fn is_jti_blacklisted(redis: &cache::RedisClient, jti: &str) -> bool {
    match redis.cache_get::<bool>("blacklist", jti).await {
        Ok(Some(true)) => true,
        Ok(_) => false,
        Err(e) => {
            tracing::warn!(error = %e, "Redis blacklist check failed — rejecting JWT (fail-closed)");
            true
        }
    }
}

/// Drop guard that decrements the active connection counter,
/// ensuring cleanup happens even if handle_socket panics.
struct WsGuard;

impl Drop for WsGuard {
    fn drop(&mut self) {
        let _prev = ACTIVE_WS_CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
        let remaining = _prev.saturating_sub(1);
        ::metrics::gauge!("ws_active_connections").set(remaining as f64);
    }
}

/// Drop guard that decrements the per-user connection count.
struct WsUserGuard {
    user_id: String,
    connections: Arc<Mutex<HashMap<String, usize>>>,
}

impl Drop for WsUserGuard {
    fn drop(&mut self) {
        // Best-effort: spawn a blocking task since Drop runs in any context
        let uid = self.user_id.clone();
        let conns = self.connections.clone();
        tokio::spawn(async move {
            let mut map = conns.lock().await;
            if let Some(count) = map.get_mut(&uid) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    map.remove(&uid);
                }
            }
        });
    }
}

/// Run the WebSocket session: subscribe to Redis events and forward to client.
async fn handle_socket(
    mut socket: WebSocket,
    redis: Arc<cache::RedisClient>,
    ws_idle_timeout: Duration,
    ws_shutdown: Arc<Notify>,
    user_connections: Arc<Mutex<HashMap<String, usize>>>,
    user_id: Option<String>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    ACTIVE_WS_CONNECTIONS.fetch_add(1, Ordering::SeqCst);
    ::metrics::gauge!("ws_active_connections").increment(1.0);
    let _ws_guard = WsGuard;
    let _user_guard = user_id.map(|uid| WsUserGuard {
        user_id: uid,
        connections: user_connections,
    });

    let subscriber = match redis.subscribe(LIVE_EVENTS_CHANNEL).await {
        Ok(rx) => rx,
        Err(e) => {
            tracing::warn!(error = %e, "failed to subscribe to live events");
            let _ = socket.close().await;
            return;
        }
    };

    let mut subscriber = subscriber;
    loop {
        tokio::select! {
            // Shutdown signal — server is stopping
            _ = ws_shutdown.notified() => {
                tracing::info!("WS connection closing — server shutdown");
                let _ = socket.close().await;
                break;
            }

            // Forward Redis message to WebSocket, with idle timeout
            msg = tokio::time::timeout(ws_idle_timeout, subscriber.recv()) => {
                match msg {
                    Ok(Some(PubSubMessage { payload, .. })) => {
                        if let Err(e) = socket.send(Message::Text(payload.into())).await {
                            tracing::info!(error = %e, "WS send error — client disconnected");
                            break;
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("pubsub channel closed");
                        break;
                    }
                    Err(_) => {
                        tracing::info!("WS idle timeout — closing connection");
                        let _ = socket.close().await;
                        break;
                    }
                }
            }

            // Handle incoming WS messages (ping/pong/close/binary)
            ws_msg = socket.recv() => {
                match ws_msg {
                    Some(Ok(Message::Ping(_))) => {
                        // Axum handles Pong automatically
                    }
                    Some(Ok(Message::Close(_))) => {
                        tracing::info!("client sent close frame");
                        break;
                    }
                    Some(Ok(Message::Binary(_))) => {
                        // Ignore unexpected binary frames
                        tracing::debug!("received unexpected binary frame — ignoring");
                    }
                    Some(Ok(Message::Text(_))) => {
                        // Client-to-server text messages are not expected in this design
                        tracing::debug!("received unexpected text message — ignoring");
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Ignore unsolicited pongs
                    }
                    Some(Err(e)) => {
                        tracing::info!(error = %e, "WS protocol error — client disconnected");
                        break;
                    }
                    None => {
                        // WebSocket closed
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics;
    use axum::routing::get;
    use tower::ServiceExt;

    #[test]
    fn live_event_serde_health_update() {
        let event = LiveEvent::HealthUpdate {
            service: "redis".into(),
            status: "ok".into(),
            detail: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("health_update"));
        assert!(json.contains("redis"));

        let deserialized: LiveEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            LiveEvent::HealthUpdate {
                service, status, ..
            } => {
                assert_eq!(service, "redis");
                assert_eq!(status, "ok");
            }
            _ => panic!("expected HealthUpdate"),
        }
    }

    #[test]
    fn live_event_serde_auth_event() {
        let event = LiveEvent::AuthEvent {
            kind: "login".into(),
            email: Some("user@example.com".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("auth_event"));

        let deserialized: LiveEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            LiveEvent::AuthEvent { kind, email } => {
                assert_eq!(kind, "login");
                assert_eq!(email, Some("user@example.com".to_string()));
            }
            _ => panic!("expected AuthEvent"),
        }
    }

    #[test]
    fn live_event_serde_connection_status() {
        let event = LiveEvent::ConnectionStatus {
            status: "connected".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("connection_status"));

        let deserialized: LiveEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            LiveEvent::ConnectionStatus { status } => {
                assert_eq!(status, "connected");
            }
            _ => panic!("expected ConnectionStatus"),
        }
    }

    #[tokio::test]
    async fn ws_returns_non_2xx_when_redis_disabled() {
        use axum::http::Request;
        let app = axum::Router::new()
            .route("/live", get(ws_handler))
            .with_state(Arc::new(crate::AppState {
                db: crate::DbStatus::NotConfigured,
                redis: None,
                auth: None,
                storage: None,
                sidecar: crate::PythonSidecar::new(
                    std::path::PathBuf::from("/tmp/nonexistent.sock"),
                    std::time::Duration::from_secs(1),
                    0,
                ),
                prometheus_handle: metrics::init_metrics_recorder(),
                gauge_task: None,
                feature_flags: None,
                ws_connection_permits: std::sync::Arc::new(tokio::sync::Semaphore::new(100)),
                ws_idle_timeout: std::time::Duration::from_secs(300),
                ws_shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
                ws_user_connections: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
                ws_per_user_max: 10,
            }));

        // Without Upgrade header, WebSocketUpgrade extractor returns 400
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/live")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Should not succeed (2xx) — Redis is disabled
        assert!(resp.status().is_client_error());
    }

    #[test]
    fn broadcast_event_does_not_panic_when_redis_none() {
        // This test validates the fix for D3 (Redis None panic):
        // broadcast_event must return early when state.redis is None
        let state = crate::AppState {
            db: crate::DbStatus::NotConfigured,
            redis: None,
            auth: None,
            storage: None,
            sidecar: crate::PythonSidecar::new(
                std::path::PathBuf::from("/tmp/nonexistent.sock"),
                std::time::Duration::from_secs(1),
                0,
            ),
            prometheus_handle: metrics::init_metrics_recorder(),
            gauge_task: None,
            feature_flags: None,
            ws_connection_permits: std::sync::Arc::new(tokio::sync::Semaphore::new(100)),
            ws_idle_timeout: std::time::Duration::from_secs(300),
            ws_shutdown: std::sync::Arc::new(tokio::sync::Notify::new()),
            ws_user_connections: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            ws_per_user_max: 10,
        };

        let event = LiveEvent::ConnectionStatus {
            status: "connected".into(),
        };

        // Should not panic — runs immediately even though Redis is None
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            broadcast_event(&state, &event).await;
        });
    }
}
