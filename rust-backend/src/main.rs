use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

mod cache;

use cache::CacheClient;

#[derive(Clone)]
struct AppState {
    db_pool: Arc<sqlx::PgPool>,
    cache: CacheClient,
}

#[allow(dead_code)]
impl AppState {
    fn db(&self) -> &sqlx::PgPool {
        &self.db_pool
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/bare_metal".to_string());

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    log::info!("Connecting to database: {}", database_url);
    log::info!("Connecting to Redis: {}", redis_url);
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let cache = CacheClient::new(&redis_url)?;

    let state = AppState {
        db_pool: Arc::new(pool),
        cache,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/cache/example", get(cache_example))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    let port = listener.local_addr()?.port();
    
    log::info!("🚀 Server running on http://0.0.0.0:{}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check(State(_state): State<AppState>) -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

async fn cache_example(State(state): State<AppState>) -> impl IntoResponse {
    let cache_key = "example_key";
    let cache_value = "example_value";

    match state.cache.get(cache_key) {
        Ok(Some(value)) => {
            log::info!("Cache hit for key: {}", cache_key);
            return Json(json!({
                "source": "cache",
                "value": value
            }));
        }
        Ok(None) => {
            log::info!("Cache miss for key: {}, setting value", cache_key);
        }
        Err(e) => {
            log::error!("Cache error: {}", e);
        }
    }

    if let Err(e) = state.cache.set_ex(cache_key, cache_value, 3600) {
        log::error!("Failed to set cache: {}", e);
    }

    Json(json!({
        "source": "freshly_set",
        "value": cache_value
    }))
}
