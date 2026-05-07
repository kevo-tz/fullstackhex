use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Instant;

static RECORDER_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Initialize the Prometheus metrics recorder with custom histogram buckets.
/// Returns the handle for rendering metrics on the `/metrics` endpoint.
///
/// Safe to call from multiple threads — the recorder is only installed once.
pub fn init_metrics_recorder() -> PrometheusHandle {
    RECORDER_HANDLE
        .get_or_init(|| {
            let recorder = PrometheusBuilder::new()
                .set_buckets_for_metric(
                    Matcher::Full("http_request_duration_seconds".to_string()),
                    &[
                        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                    ],
                )
                .expect("invalid http buckets")
                .set_buckets_for_metric(
                    Matcher::Full("auth_latency_seconds".to_string()),
                    &[
                        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                    ],
                )
                .expect("invalid auth latency buckets")
                .build_recorder();
            let handle = recorder.handle();
            metrics::set_global_recorder(recorder).expect("failed to set global metrics recorder");
            handle
        })
        .clone()
}

/// Render all collected metrics in Prometheus text exposition format.
pub fn render_metrics(handle: &PrometheusHandle) -> String {
    handle.render()
}

/// Normalize a request path to a bounded route label.
///
/// Prevents cardinality explosion from path parameters by collapsing
/// all unrecognized paths to `"unknown"`.
pub fn normalize_route(path: &str) -> &'static str {
    match path {
        "/health" => "/health",
        "/health/db" => "/health/db",
        "/health/redis" => "/health/redis",
        "/health/storage" => "/health/storage",
        "/health/python" => "/health/python",
        "/metrics" => "/metrics",
        "/metrics/python" => "/metrics/python",
        "/auth/login" => "/auth/login",
        "/auth/register" => "/auth/register",
        "/auth/logout" => "/auth/logout",
        "/auth/refresh" => "/auth/refresh",
        "/auth/me" => "/auth/me",
        "/auth/oauth/{provider}" => "/auth/oauth",
        "/auth/oauth/{provider}/callback" => "/auth/oauth/callback",
        "/storage/{key}" => "/storage/key",
        "/storage" => "/storage",
        "/storage/presign" => "/storage/presign",
        _ => "unknown",
    }
}

/// Axum middleware that records `http_requests_total` and
/// `http_request_duration_seconds` for every request.
pub async fn track_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or_else(|| normalize_route(&path))
        .to_owned();

    let response = next.run(request).await;
    let status = response.status().as_u16().to_string();
    let duration = start.elapsed().as_secs_f64();

    metrics::counter!(
        "http_requests_total",
        "method" => method.clone(),
        "route" => route.clone(),
        "status" => status
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "route" => route
    )
    .record(duration);

    response
}

/// Spawn a background task that updates the `db_pool_connections` gauge
/// every 15 seconds.
///
/// Returns an [`AbortHandle`] so the task can be stopped cleanly on shutdown.
pub fn spawn_pool_gauge_task(pool: PgPool) -> tokio::task::AbortHandle {
    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            let size = pool.size();
            let idle = pool.num_idle() as u32;
            let active = size.saturating_sub(idle);
            metrics::gauge!("db_pool_connections", "state" => "idle").set(idle as f64);
            metrics::gauge!("db_pool_connections", "state" => "used").set(active as f64);
        }
    });
    handle.abort_handle()
}
