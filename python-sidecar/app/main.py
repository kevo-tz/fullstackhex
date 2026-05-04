from datetime import datetime, timezone
from fastapi import FastAPI, Request, Response
import hmac
import hashlib
import logging
import json
import os
import sys
import time

from prometheus_client import (
    Counter,
    Histogram,
    generate_latest,
    CONTENT_TYPE_LATEST,
)

app = FastAPI()

# Prometheus metrics
PYTHON_REQUESTS_TOTAL = Counter(
    "python_requests_total",
    "Total HTTP requests",
    ["method", "endpoint", "status"],
)
PYTHON_REQUEST_DURATION = Histogram(
    "python_request_duration_seconds",
    "HTTP request duration",
    ["method", "endpoint"],
    buckets=[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0],
)


class JsonFormatter(logging.Formatter):
    def format(self, record: logging.LogRecord) -> str:
        ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
        obj = {
            "timestamp": ts,
            "level": record.levelname.lower(),
            "target": record.name,
            "message": record.getMessage(),
        }
        if hasattr(record, "trace_id"):
            obj["trace_id"] = record.trace_id
        if record.exc_info and record.exc_info[1]:
            obj["error"] = str(record.exc_info[1])
        return json.dumps(obj)


def setup_logging() -> None:
    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(JsonFormatter())
    root = logging.getLogger()
    # Clear uvicorn handlers to avoid duplicate output
    root.handlers.clear()
    root.addHandler(handler)
    root.setLevel(logging.INFO)


setup_logging()
logger = logging.getLogger("python-sidecar")


@app.middleware("http")
async def trace_id_middleware(request: Request, call_next):
    trace_id = request.headers.get("x-trace-id", "")
    start = time.monotonic()
    response = await call_next(request)
    duration = time.monotonic() - start
    duration_ms = int(duration * 1000)
    logger.info(
        f"{request.method} {request.url.path} → {response.status_code}",
        extra={
            "trace_id": trace_id,
            "duration_ms": duration_ms,
            "status_code": response.status_code,
        },
    )
    # Record Prometheus metrics
    endpoint = request.url.path
    status = str(response.status_code)
    PYTHON_REQUESTS_TOTAL.labels(method=request.method, endpoint=endpoint, status=status).inc()
    PYTHON_REQUEST_DURATION.labels(method=request.method, endpoint=endpoint).observe(duration)
    return response


@app.middleware("http")
async def hmac_auth_middleware(request: Request, call_next):
    path = request.url.path
    # Skip HMAC for public routes
    if path in ("/health", "/metrics"):
        return await call_next(request)

    shared_secret = os.environ.get("SIDECAR_SHARED_SECRET", "")
    if not shared_secret:
        # Fail closed — never trust auth headers if shared secret is missing
        return Response(
            content=json.dumps({"error": "SIDECAR_SHARED_SECRET not configured — rejecting all requests"}),
            status_code=401,
            media_type="application/json",
        )

    user_id = request.headers.get("X-User-Id", "")
    email = request.headers.get("X-User-Email", "")
    name = request.headers.get("X-User-Name", "")
    signature = request.headers.get("X-Auth-Signature", "")

    if not all([user_id, email, signature]):
        return Response(
            content=json.dumps({"error": "Missing auth headers"}),
            status_code=401,
            media_type="application/json",
        )

    # Compute expected signature: HMAC-SHA256(secret, "user_id|email|name")
    payload = f"{user_id}|{email}|{name}"
    expected = hmac.new(
        shared_secret.encode("utf-8"),
        payload.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()

    if not hmac.compare_digest(expected, signature):
        return Response(
            content=json.dumps({"error": "Invalid auth signature"}),
            status_code=401,
            media_type="application/json",
        )

    return await call_next(request)


@app.get("/health")
def health(request: Request) -> dict[str, str]:
    trace_id = request.headers.get("x-trace-id", "")
    logger.info("health check", extra={"trace_id": trace_id})
    # Bump this version together with VERSION file at repo root
    return {"status": "ok", "service": "python-sidecar", "version": "0.7.0"}


@app.get("/metrics")
def metrics() -> Response:
    return Response(
        content=generate_latest(),
        media_type=CONTENT_TYPE_LATEST,
    )
