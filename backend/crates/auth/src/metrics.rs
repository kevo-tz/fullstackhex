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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware;
    use axum::Router;
    use tower::ServiceExt;

    fn test_app() -> Router {
        Router::new()
            .route(
                "/{status}",
                axum::routing::get(
                    |axum::extract::Path(status): axum::extract::Path<u16>| async move {
                        StatusCode::from_u16(status).unwrap_or(StatusCode::OK)
                    },
                ),
            )
            .layer(middleware::from_fn(track_auth_metrics))
    }

    #[tokio::test]
    async fn returns_200_response() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/200")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn returns_400_response() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/400")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn returns_500_response() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/500")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
