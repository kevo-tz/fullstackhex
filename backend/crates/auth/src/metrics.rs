use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;

pub async fn track_auth_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    let status = response.status().as_u16();
    let latency_s = start.elapsed().as_secs_f64();

    metrics::counter!("auth_requests_total", "method" => method.clone(), "path" => path.clone())
        .increment(1);
    metrics::histogram!("auth_latency_seconds", "method" => method, "path" => path)
        .record(latency_s);

    if status >= 400 {
        let error_type = if status >= 500 { "server" } else { "client" };
        metrics::counter!("auth_errors_total", "error_type" => error_type.to_string(), "status" => status.to_string())
            .increment(1);
    }

    response
}
