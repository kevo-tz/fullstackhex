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

#[derive(Clone)]
struct AppState {
    db_pool: Arc<sqlx::PgPool>,
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

    let database_url = std::env::var("RUST_SERVICE_DB_URL")
        .unwrap_or_else(|_| "postgres://localhost/bare_metal".to_string());

    log::info!("Connecting to database: {}", database_url);
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let state = AppState {
        db_pool: Arc::new(pool),
    };

    let app = Router::new()
        .route("/health", get(health_check))
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
