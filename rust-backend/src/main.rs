use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use prometheus::{Encoder, TextEncoder, register_gauge, register_counter, Counter, Gauge};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::LazyLock;
use tokio::signal;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cache;
use cache::CacheClient;

// Prometheus metrics
static HTTP_REQUESTS_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    register_counter!("http_requests_total", "Total number of HTTP requests").unwrap()
});

static DB_CONNECTION_POOL_SIZE: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge!("db_connection_pool_size", "Current number of DB connections in pool").unwrap()
});

#[derive(Clone)]
struct AppState {
    db_pool: sqlx::PgPool,
    cache: CacheClient,
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    HTTP_REQUESTS_TOTAL.inc();
    
    // Check DB
    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.db_pool).await.is_ok();
    DB_CONNECTION_POOL_SIZE.set(state.db_pool.size() as f64);
    
    // Check Redis
    let redis_ok = state.cache.exists("health_check").await.is_ok();
    
    Json(json!({
        "status": if db_ok && redis_ok { "ok" } else { "degraded" },
        "database": db_ok,
        "redis": redis_ok
    }))
}

async fn cache_example(State(state): State<AppState>) -> impl IntoResponse {
    HTTP_REQUESTS_TOTAL.inc();
    
    let cache_key = "example_key";
    let cache_value = "example_value";

    match state.cache.get(cache_key).await {
        Ok(Some(value)) => {
            tracing::info!("Cache hit for key: {}", cache_key);
            return Json(json!({
                "source": "cache",
                "value": value
            }));
        }
        Ok(None) => {
            tracing::info!("Cache miss for key: {}, setting value", cache_key);
        }
        Err(e) => {
            tracing::error!("Cache error: {}", e);
        }
    }

    if let Err(e) = state.cache.set_ex(cache_key, cache_value, 3600).await {
        tracing::error!("Failed to set cache: {}", e);
    }

    Json(json!({
        "source": "freshly_set",
        "value": cache_value
    }))
}

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    
    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        buffer,
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/bare_metal".to_string());

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    tracing::info!("Connecting to database: {}", database_url);
    tracing::info!("Connecting to Redis: {}", redis_url);

    // Optimized connection pool
    let pool = PgPoolOptions::new()
        .max_connections(50)
        .min_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    let cache = CacheClient::new(&redis_url);

    let state = AppState {
        db_pool: pool,
        cache,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/cache/example", get(cache_example))
        .route("/metrics", get(metrics_handler))
        .layer(CompressionLayer::new()) // Brotli/Gzip compression
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    tracing::info!("🚀 Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
