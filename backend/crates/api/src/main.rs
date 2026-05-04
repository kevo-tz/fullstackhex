use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    if let Err(e) = dotenvy::dotenv() {
        tracing::warn!(error = %e, "failed to load .env file — continuing with existing environment");
    }

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .with_current_span(false)
        .init();

    let prometheus_handle = api::metrics::init_metrics_recorder();
    let (app, state) = api::router(prometheus_handle).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "failed to initialize application");
        std::process::exit(1);
    });

    let addr: SocketAddr = "0.0.0.0:8001".parse().unwrap();
    tracing::info!(%addr, "listening");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    // Graceful shutdown on SIGTERM (Docker standard) + SIGINT (Ctrl-C)
    let shutdown = async {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = sigterm.recv() => {},
            _ = tokio::signal::ctrl_c() => {},
        }
        tracing::info!("received shutdown signal, draining connections");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .unwrap();

    if let Some(handle) = &state.gauge_task {
        handle.abort();
    }
}
