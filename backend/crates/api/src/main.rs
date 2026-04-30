use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = api::router().await;

    let addr: SocketAddr = "0.0.0.0:8001".parse().unwrap();
    println!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
