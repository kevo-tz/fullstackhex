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
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use cache::pubsub::PubSubMessage;
use serde::{Deserialize, Serialize};

use futures_util::sink::SinkExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
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
    let redis = match &state.health.redis {
        Some(r) => r,
        None => {
            tracing::warn!("broadcast_event: Redis not configured — dropping event");
            return;
        }
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

/// Result of WebSocket connection validation.
enum WsConnectionOutcome {
    Permit {
        redis: Arc<cache::RedisClient>,
        permit: tokio::sync::OwnedSemaphorePermit,
        user_id: Option<String>,
    },
    Reject(axum::response::Response),
}

/// Validate a WebSocket connection request and return the resources needed
/// for a successful upgrade, or an HTTP error response.
///
/// Extracted from `ws_handler` so the business logic can be unit-tested
/// without requiring the Axum `WebSocketUpgrade` extractor.
async fn validate_ws_connection(headers: &HeaderMap, state: &AppState) -> WsConnectionOutcome {
    // Validate Origin when ALLOWED_ORIGIN is configured — prevents cross-site
    // WebSocket hijacking even when session cookies are present.
    if let Some(ref allowed) = state.allowed_origin {
        let origin = headers
            .get(axum::http::header::ORIGIN)
            .and_then(|v| v.to_str().ok());
        match origin {
            Some(o) if o == allowed => {}
            Some(_) | None => {
                return WsConnectionOutcome::Reject(
                    (StatusCode::FORBIDDEN, "{\"error\":\"Origin not allowed\"}").into_response(),
                );
            }
        }
    }

    let maybe_user_id = if state.auth.is_some() {
        cookie_authenticated(headers, state).await
    } else {
        None
    };

    if state.auth.is_some() && maybe_user_id.is_none() {
        return WsConnectionOutcome::Reject(
            (
                StatusCode::UNAUTHORIZED,
                "{\"error\":\"Authentication required\"}",
            )
                .into_response(),
        );
    }

    let redis = match &state.health.redis {
        Some(r) => r.clone(),
        None => {
            return WsConnectionOutcome::Reject(
                (
                    StatusCode::NOT_FOUND,
                    "{\"error\":\"WebSocket not available — Redis is disabled\"}",
                )
                    .into_response(),
            );
        }
    };

    let permit = match state.ws.connection_permits.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            return WsConnectionOutcome::Reject(
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "{\"error\":\"Server at capacity — too many WebSocket connections\"}",
                )
                    .into_response(),
            );
        }
    };

    if let Some(ref uid) = maybe_user_id {
        let mut conns = state.ws.user_connections.write().unwrap();
        let current = *conns.get(uid).unwrap_or(&0);
        if current >= state.ws.per_user_max {
            return WsConnectionOutcome::Reject(
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    "{\"error\":\"Too many connections from this user\"}",
                )
                    .into_response(),
            );
        }
        conns.insert(uid.clone(), current + 1);
    }

    WsConnectionOutcome::Permit {
        redis,
        permit,
        user_id: maybe_user_id,
    }
}

/// Handle WebSocket upgrade requests.
///
/// Returns 404 Not Found when Redis is disabled — the frontend falls back
/// to HTTP polling automatically. When auth is configured (`state.auth` is
/// `Some`), the endpoint requires a valid session cookie. Returns 401
/// Unauthorized when auth is configured but no valid credentials are provided.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match validate_ws_connection(&headers, &state).await {
        WsConnectionOutcome::Permit {
            redis,
            permit,
            user_id,
        } => {
            let idle_timeout = state.ws.idle_timeout;
            let ws_shutdown = state.ws.shutdown.clone();
            let user_connections = state.ws.user_connections.clone();
            ws.on_upgrade(move |socket| {
                handle_socket(
                    socket,
                    redis,
                    idle_timeout,
                    ws_shutdown,
                    user_connections,
                    user_id,
                    permit,
                )
            })
        }
        WsConnectionOutcome::Reject(response) => response,
    }
}

/// Authenticate a WebSocket connection via session cookie.
///
/// Extracts the `session=` cookie from the `Cookie` header, deserializes the
/// `Session` struct from Redis, and returns the user_id on success.
/// Returns `None` if the cookie is missing or the session is not found.
async fn cookie_authenticated(headers: &HeaderMap, state: &AppState) -> Option<String> {
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

    let redis = match &state.health.redis {
        Some(r) => r.clone(),
        None => return None,
    };

    let session: Option<cache::session::Session> =
        match redis.cache_get("session", &session_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "Redis session lookup failed in cookie_authenticated");
                None
            }
        };

    if let Some(s) = session {
        Some(s.user_id)
    } else {
        None
    }
}

/// Drop guard that decrements the active connection counter,
/// ensuring cleanup happens even if handle_socket panics.
struct WsGuard;

impl Drop for WsGuard {
    fn drop(&mut self) {
        let prev = ACTIVE_WS_CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
        let remaining = prev.saturating_sub(1);
        ::metrics::gauge!("ws_active_connections").set(remaining as f64);
    }
}

/// Drop guard that decrements the per-user connection count.
struct WsUserGuard {
    user_id: String,
    connections: Arc<RwLock<HashMap<String, usize>>>,
}

impl Drop for WsUserGuard {
    fn drop(&mut self) {
        let uid = &self.user_id;
        let mut map = self.connections.write().unwrap();
        if let Some(count) = map.get_mut(uid) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                map.remove(uid);
            }
        }
    }
}

/// Run the WebSocket session: subscribe to Redis events and forward to client.
async fn handle_socket(
    mut socket: WebSocket,
    redis: Arc<cache::RedisClient>,
    ws_idle_timeout: Duration,
    ws_shutdown: Arc<Notify>,
    user_connections: Arc<RwLock<HashMap<String, usize>>>,
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
                        // Use a short timeout on send() to avoid blocking the subscriber
                        // loop when the client's TCP buffer is full. Drop the message
                        // instead of stalling all event delivery.
                        match tokio::time::timeout(
                            Duration::from_millis(100),
                            socket.send(Message::Text(payload.into())),
                        ).await {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => {
                                tracing::info!(error = %e, "WS send error — client disconnected");
                                break;
                            }
                            Err(_) => {
                                tracing::warn!("WS backpressure — dropping message");
                            }
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
            .with_state(Arc::new(test_state()));

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
        // broadcast_event must return early when state.health.redis is None
        let state = test_state();

        let event = LiveEvent::ConnectionStatus {
            status: "connected".into(),
        };

        // Should not panic — runs immediately even though Redis is None
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            broadcast_event(&state, &event).await;
        });
    }

    #[test]
    fn ws_user_guard_decrements_on_drop() {
        let map: Arc<RwLock<HashMap<String, usize>>> = Arc::new(RwLock::new(HashMap::new()));
        {
            let mut m = map.write().unwrap();
            m.insert("user1".into(), 3usize);
        }
        {
            let _user_guard = WsUserGuard {
                user_id: "user1".into(),
                connections: map.clone(),
            };
        }
        assert_eq!(map.read().unwrap().get("user1"), Some(&2usize));
    }

    #[test]
    fn ws_user_guard_removes_key_when_zero() {
        let map: Arc<RwLock<HashMap<String, usize>>> = Arc::new(RwLock::new(HashMap::new()));
        {
            let mut m = map.write().unwrap();
            m.insert("user1".into(), 1usize);
        }
        {
            let _user_guard = WsUserGuard {
                user_id: "user1".into(),
                connections: map.clone(),
            };
        }
        assert!(map.read().unwrap().get("user1").is_none());
    }

    #[test]
    fn ws_user_guard_is_noop_for_unknown_user() {
        let map: Arc<RwLock<HashMap<String, usize>>> = Arc::new(RwLock::new(HashMap::new()));
        {
            let mut m = map.write().unwrap();
            m.insert("user1".into(), 3usize);
        }
        {
            let _user_guard = WsUserGuard {
                user_id: "unknown_user".into(),
                connections: map.clone(),
            };
        }
        assert_eq!(map.read().unwrap().get("user1"), Some(&3usize));
    }

    #[test]
    fn ws_user_guard_is_noop_on_empty_map() {
        let map: Arc<RwLock<HashMap<String, usize>>> = Arc::new(RwLock::new(HashMap::new()));
        {
            let _user_guard = WsUserGuard {
                user_id: "user1".into(),
                connections: map.clone(),
            };
        }
        assert!(map.read().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cookie_authenticated_returns_none_without_cookie() {
        let headers = HeaderMap::new();
        let state = test_state();
        assert!(cookie_authenticated(&headers, &state).await.is_none());
    }

    #[tokio::test]
    async fn cookie_authenticated_returns_none_when_redis_none() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "session=abc123".parse().unwrap());
        let state = test_state();
        assert!(cookie_authenticated(&headers, &state).await.is_none());
    }

    #[tokio::test]
    async fn cookie_authenticated_returns_none_for_empty_session() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "session=".parse().unwrap());
        let state = test_state();
        assert!(cookie_authenticated(&headers, &state).await.is_none());
    }

    #[tokio::test]
    async fn validate_connection_returns_401_when_auth_configured_no_cookie() {
        let headers = HeaderMap::new();
        let state = AppState {
            health: Arc::new(crate::HealthState {
                db: crate::DbStatus::NotConfigured,
                redis: None,
                sidecar: crate::PythonSidecar::new(
                    std::path::PathBuf::from("/tmp/nonexistent.sock"),
                    Duration::from_secs(1),
                    0,
                ),
                gauge_task: None,
                feature_flags: domain::FeatureFlags {
                    maintenance_mode: false,
                },
            }),
            ws: Arc::new(crate::WebSocketState {
                connection_permits: Arc::new(tokio::sync::Semaphore::new(100)),
                idle_timeout: Duration::from_secs(300),
                shutdown: Arc::new(tokio::sync::Notify::new()),
                user_connections: Arc::new(std::sync::RwLock::new(HashMap::new())),
                per_user_max: 10,
            }),
            auth: Some(Arc::new(auth::AuthService::new(auth::AuthConfig {
                jwt_secret: "test".to_string(),
                jwt_issuer: "test".to_string(),
                jwt_expiry: 900,
                refresh_expiry: 604800,
                auth_mode: auth::AuthMode::Cookie,
                google_client_id: None,
                google_client_secret: None,
                github_client_id: None,
                github_client_secret: None,
                oauth_redirect_url: None,
                sidecar_shared_secret: None,
                fail_open_on_redis_error: true,
                rate_limits: Default::default(),
                cookie_secure: false,
            }))),
            storage: None,
            prometheus_handle: metrics::init_metrics_recorder(),
            allowed_origin: None,
        };
        match validate_ws_connection(&headers, &state).await {
            WsConnectionOutcome::Reject(response) => {
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            }
            _ => panic!("expected Reject with 401"),
        }
    }

    #[tokio::test]
    async fn validate_connection_returns_404_when_redis_disabled() {
        let headers = HeaderMap::new();
        let state = test_state();
        match validate_ws_connection(&headers, &state).await {
            WsConnectionOutcome::Reject(response) => {
                assert_eq!(response.status(), StatusCode::NOT_FOUND);
            }
            _ => panic!("expected Reject with 404"),
        }
    }

    async fn state_with_redis(permits: usize) -> Option<AppState> {
        let url = std::env::var("REDIS_URL").ok()?;
        let redis = cache::RedisClient::new(&url, "test-ws-validate")
            .await
            .ok()?;
        Some(AppState {
            health: Arc::new(crate::HealthState {
                db: crate::DbStatus::NotConfigured,
                redis: Some(Arc::new(redis)),
                sidecar: crate::PythonSidecar::new(
                    std::path::PathBuf::from("/tmp/nonexistent.sock"),
                    Duration::from_secs(1),
                    0,
                ),
                gauge_task: None,
                feature_flags: domain::FeatureFlags {
                    maintenance_mode: false,
                },
            }),
            ws: Arc::new(crate::WebSocketState {
                connection_permits: Arc::new(tokio::sync::Semaphore::new(permits)),
                idle_timeout: Duration::from_secs(300),
                shutdown: Arc::new(tokio::sync::Notify::new()),
                user_connections: Arc::new(std::sync::RwLock::new(HashMap::new())),
                per_user_max: 10,
            }),
            auth: None,
            storage: None,
            prometheus_handle: metrics::init_metrics_recorder(),
            allowed_origin: None,
        })
    }

    #[tokio::test]
    async fn validate_connection_returns_503_when_semaphore_exhausted() {
        let Some(state) = state_with_redis(0).await else {
            eprintln!("SKIP: REDIS_URL not set or unreachable");
            return;
        };
        let headers = HeaderMap::new();
        match validate_ws_connection(&headers, &state).await {
            WsConnectionOutcome::Reject(response) => {
                assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            }
            _ => panic!("expected Reject with 503"),
        }
    }

    #[tokio::test]
    async fn validate_connection_returns_permit_when_all_ok() {
        let Some(state) = state_with_redis(100).await else {
            eprintln!("SKIP: REDIS_URL not set or unreachable");
            return;
        };
        let headers = HeaderMap::new();
        match validate_ws_connection(&headers, &state).await {
            WsConnectionOutcome::Permit { .. } => {}
            WsConnectionOutcome::Reject(response) => {
                panic!("expected Permit, got {}", response.status());
            }
        }
    }

    fn test_state() -> crate::AppState {
        use std::collections::HashMap;
        use std::sync::Arc;
        use std::time::Duration;

        crate::AppState {
            health: Arc::new(crate::HealthState {
                db: crate::DbStatus::NotConfigured,
                redis: None,
                sidecar: crate::PythonSidecar::new(
                    std::path::PathBuf::from("/tmp/nonexistent.sock"),
                    Duration::from_secs(1),
                    0,
                ),
                gauge_task: None,
                feature_flags: domain::FeatureFlags {
                    maintenance_mode: false,
                },
            }),
            ws: Arc::new(crate::WebSocketState {
                connection_permits: Arc::new(tokio::sync::Semaphore::new(100)),
                idle_timeout: Duration::from_secs(300),
                shutdown: Arc::new(tokio::sync::Notify::new()),
                user_connections: Arc::new(std::sync::RwLock::new(HashMap::new())),
                per_user_max: 10,
            }),
            auth: None,
            storage: None,
            prometheus_handle: metrics::init_metrics_recorder(),
            allowed_origin: None,
        }
    }
}
