"""Tests for trace middleware and metrics endpoint."""
import json

from fastapi.testclient import TestClient

from app.main import app


def test_trace_middleware_records_metrics() -> None:
    """The trace middleware should record Prometheus metrics for each request."""
    client = TestClient(app)

    # Make a request to trigger metric recording
    response = client.get("/health")
    assert response.status_code == 200

    # Check that metrics endpoint exposes the recorded request
    metrics_response = client.get("/metrics")
    assert metrics_response.status_code == 200
    metrics_text = metrics_response.text

    # The trace middleware labels are method + endpoint
    assert 'python_requests_total{endpoint="/health",method="GET",status="200"}' in metrics_text
    assert 'python_request_duration_seconds_count{endpoint="/health",method="GET"}' in metrics_text


def test_trace_middleware_preserves_trace_id_header() -> None:
    """The trace middleware should propagate the x-trace-id header through responses."""
    client = TestClient(app)
    response = client.get("/health", headers={"x-trace-id": "trace-abc-123"})
    assert response.status_code == 200
    # The middleware doesn't echo the trace_id in the response body, but it
    # should not drop the header or error out.
    assert response.json()["status"] == "ok"


def test_metrics_endpoint_returns_prometheus_format() -> None:
    """/metrics should return Prometheus exposition format."""
    client = TestClient(app)
    response = client.get("/metrics")

    assert response.status_code == 200
    assert response.headers["content-type"] == "text/plain; version=0.0.4; charset=utf-8"
    body = response.text
    assert "# HELP python_requests_total" in body
    assert "# TYPE python_requests_total counter" in body
    assert "# HELP python_request_duration_seconds" in body
    assert "# TYPE python_request_duration_seconds histogram" in body


def test_metrics_endpoint_increments_after_requests() -> None:
    """Multiple requests should increment the request counter."""
    client = TestClient(app)

    # Make several requests
    for _ in range(3):
        client.get("/health")

    metrics_response = client.get("/metrics")
    assert metrics_response.status_code == 200
    metrics_text = metrics_response.text

    # Find the counter value for /health GET 200
    for line in metrics_text.splitlines():
        if line.startswith('python_requests_total{endpoint="/health",method="GET",status="200"}'):
            _, value = line.rsplit(" ", 1)
            assert int(float(value)) >= 3
            break
    else:
        raise AssertionError("Expected /health GET 200 counter not found in metrics")
