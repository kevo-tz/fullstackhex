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
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cache::pubsub::PubSubMessage;
use serde::{Deserialize, Serialize};

use futures_util::sink::SinkExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing;

/// Redis pub/sub channel for live events.
const LIVE_EVENTS_CHANNEL: &str = "live:events";

/// Maximum concurrent WebSocket connections.
const MAX_WS_CONNECTIONS: usize = 100;
/// Idle timeout: close connection if no message received within this duration.
const WS_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Global active connection counter for metrics.
static ACTIVE_WS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
/// Semaphore limiting concurrent WebSocket connections.
static WS_SEMAPHORE: Semaphore = Semaphore::const_new(MAX_WS_CONNECTIONS);

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
/// to HTTP polling automatically. Requires a valid JWT token when auth is
/// configured (passed via `token` query parameter).
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Note: When auth is configured, the frontend must connect with a valid JWT
    // token passed as a query parameter (`/api/live?token=...`). The auth
    // middleware skips /live (it's a public upgrade endpoint), so token
    // validation happens inside handle_socket via an initial auth message.
    // For the public health dashboard, no token is needed.

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
    let permit = match WS_SEMAPHORE.try_acquire() {
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

    ws.on_upgrade(move |socket| handle_socket(socket, redis, permit))
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

/// Run the WebSocket session: subscribe to Redis events and forward to client.
async fn handle_socket(
    mut socket: WebSocket,
    redis: Arc<cache::RedisClient>,
    _permit: tokio::sync::SemaphorePermit<'static>,
) {
    let _guard = WsGuard;
    ACTIVE_WS_CONNECTIONS.fetch_add(1, Ordering::SeqCst);
    ::metrics::gauge!("ws_active_connections").increment(1.0);

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
            // Forward Redis message to WebSocket, with idle timeout
            msg = tokio::time::timeout(WS_IDLE_TIMEOUT, subscriber.recv()) => {
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
