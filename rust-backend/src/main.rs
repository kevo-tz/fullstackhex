use axum::{
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::net::SocketAddr;

async fn hello() -> Json<Value> {
    Json(json!({
        "message": "Hello from Rust!",
        "service": "rust-backend",
        "status": "ok"
    }))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "healthy" }))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(hello))
        .route("/health", get(health));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    println!("🚀 Rust backend running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
