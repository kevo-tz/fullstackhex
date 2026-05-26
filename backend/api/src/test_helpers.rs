use crate::metrics;
use crate::{AppState, DbStatus, HealthState, WebSocketState};
use py_sidecar::PythonSidecar;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Create a default test AppState with all optional features disabled.
pub fn new_test_state() -> AppState {
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
            feature_flags: domain::FeatureFlags { maintenance_mode: false },
        }),
        ws: Arc::new(WebSocketState {
            connection_permits: Arc::new(tokio::sync::Semaphore::new(100)),
            idle_timeout: Duration::from_secs(300),
            shutdown: Arc::new(tokio::sync::Notify::new()),
            user_connections: Arc::new(RwLock::new(HashMap::new())),
            per_user_max: 10,
        }),
        auth: None,
        storage: None,
        prometheus_handle: metrics::init_metrics_recorder(),
        allowed_origin: None,
    }
}
