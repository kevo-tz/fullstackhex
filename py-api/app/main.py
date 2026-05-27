from contextlib import asynccontextmanager
from datetime import datetime, timezone
from fastapi import FastAPI, Request, Response
import hmac
import hashlib
from importlib.metadata import version
import logging
import json
import os
import re
import sys
import time
from typing import Awaitable, Callable

import redis.asyncio as aioredis

from prometheus_client import (
    Counter,
    Histogram,
    generate_latest,
    CONTENT_TYPE_LATEST,
)


redis_client: aioredis.Redis | None = None


@asynccontextmanager
async def lifespan(_app: FastAPI):
    global redis_client
    setup_logging()
    register_metrics()
    redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379/0")
    redis_client = aioredis.from_url(redis_url, decode_responses=True)
    try:
        await redis_client.ping()
    except Exception as e:
        logging.warning("Redis connection failed — HMAC nonce dedup disabled: %s", e)
        redis_client = None
    yield
    if redis_client is not None:
        await redis_client.aclose()
        redis_client = None


app = FastAPI(lifespan=lifespan)


class Settings:
    """Application settings, sourced from environment at startup."""

    def __init__(self) -> None:
        secret = os.environ.get("SIDECAR_SHARED_SECRET", "")
        if not secret:
            logging.warning("SIDECAR_SHARED_SECRET is empty — all requests will be rejected")
        self.shared_secret: str = secret


settings = Settings()


def register_metrics() -> None:
    """Idempotent — metrics created at module level, kept for backward compat."""


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


# Cache py-api version at module level — avoids importlib.metadata lookup per request
try:
    PY_API_VERSION = version("py-api")
except Exception:
    PY_API_VERSION = "0.0.0"


class JsonFormatter(logging.Formatter):
    """Structured JSON log formatter for production logging."""

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
    """Configure root logger with JSON formatter for structured output."""
    root = logging.getLogger()
    # Guard against duplicate handlers on lifespan re-entry (test reset, dev reload).
    # Check for a stderr StreamHandler specifically to avoid false collisions with
    # pytest's _FileHandler (a StreamHandler subclass) which isn't ours to deduplicate.
    if any(isinstance(h, logging.StreamHandler) and h.stream is sys.stderr for h in root.handlers):
        return
    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(JsonFormatter())
    root.addHandler(handler)
    level_name = os.environ.get("PYTHON_LOG_LEVEL", "INFO").upper()
    root.setLevel(getattr(logging, level_name, logging.INFO))


logger = logging.getLogger("py-api")


@app.middleware("http")
async def hmac_auth_middleware(
    request: Request, call_next: Callable[[Request], Awaitable[Response]]
) -> Response:
    """FastAPI middleware that validates HMAC-SHA256 signatures on auth headers forwarded from the Rust backend."""
    trace_id = request.headers.get("x-trace-id", "")
    path = request.url.path
    # Skip HMAC for public routes
    if path in ("/health", "/metrics"):
        return await call_next(request)

    if not settings.shared_secret:
        logger.warning(
            "HMAC rejection: SIDECAR_SHARED_SECRET not configured",
            extra={"trace_id": trace_id},
        )
        return Response(
            content=json.dumps(
                {"error": "SIDECAR_SHARED_SECRET not configured — rejecting all requests"}
            ),
            status_code=401,
            media_type="application/json",
        )

    user_id = request.headers.get("X-User-Id", "")
    email = request.headers.get("X-User-Email", "")
    name = request.headers.get("X-User-Name", "")
    signature = request.headers.get("X-Auth-Signature", "")
    timestamp_str = request.headers.get("X-Timestamp", "")
    nonce = request.headers.get("X-Nonce", "")

    if not all([user_id, email, signature]):
        logger.warning(
            "HMAC rejection: missing auth headers",
            extra={"trace_id": trace_id, "has_user_id": bool(user_id), "has_email": bool(email)},
        )
        return Response(
            content=json.dumps({"error": "Missing auth headers"}),
            status_code=401,
            media_type="application/json",
        )

    # Validate timestamp (±30s window)
    try:
        ts = int(timestamp_str)
        now = int(time.time())
        if abs(now - ts) > 30:
            logger.warning(
                "HMAC rejection: timestamp outside window",
                extra={"trace_id": trace_id, "timestamp": ts, "skew": now - ts},
            )
            return Response(
                content=json.dumps({"error": "Request expired"}),
                status_code=401,
                media_type="application/json",
            )
    except ValueError, TypeError:
        logger.warning(
            "HMAC rejection: missing or invalid timestamp",
            extra={"trace_id": trace_id},
        )
        return Response(
            content=json.dumps({"error": "Missing or invalid timestamp"}),
            status_code=401,
            media_type="application/json",
        )

    # Replay protection: check nonce hasn't been seen (atomic SET NX)
    if nonce and redis_client is not None:
        nonce_key = f"hmac:nonce:{nonce}"
        set_ok = await redis_client.set(nonce_key, "1", nx=True, ex=90)
        if not set_ok:
            logger.warning(
                "HMAC rejection: duplicate nonce (replay)",
                extra={"trace_id": trace_id, "nonce": nonce},
            )
            return Response(
                content=json.dumps({"error": "Duplicate request"}),
                status_code=401,
                media_type="application/json",
            )
    elif nonce and redis_client is None:
        logger.warning(
            "HMAC rejection: nonce provided but Redis unavailable — rejecting",
            extra={"trace_id": trace_id},
        )
        return Response(
            content=json.dumps({"error": "Auth service degraded"}),
            status_code=401,
            media_type="application/json",
        )
    # Compute expected signature: HMAC-SHA256(secret, JSON payload)
    # Compact separators match serde_json::to_string() from Rust side
    payload = json.dumps(
        {"user_id": user_id, "email": email, "name": name, "timestamp": ts},
        sort_keys=True,
        separators=(",", ":"),
    )
    expected = hmac.new(
        settings.shared_secret.encode("utf-8"),
        payload.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()

    if not hmac.compare_digest(expected, signature):
        logger.warning(
            "HMAC rejection: invalid signature",
            extra={
                "trace_id": trace_id,
                "user_id": user_id,
                "email": email,
            },
        )
        return Response(
            content=json.dumps({"error": "Invalid auth signature"}),
            status_code=401,
            media_type="application/json",
        )

    return await call_next(request)


_UUID_PATTERN = re.compile(r"/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")


def _normalize_endpoint(path: str) -> str:
    """Replace UUID segments with `{id}` to prevent Prometheus label cardinality explosion."""
    return _UUID_PATTERN.sub("/{id}", path)


@app.middleware("http")
async def trace_id_middleware(
    request: Request, call_next: Callable[[Request], Awaitable[Response]]
) -> Response:
    """FastAPI middleware that logs request duration and increments Prometheus counters."""
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
    endpoint = _normalize_endpoint(request.url.path)
    status = str(response.status_code)
    PYTHON_REQUESTS_TOTAL.labels(method=request.method, endpoint=endpoint, status=status).inc()
    PYTHON_REQUEST_DURATION.labels(method=request.method, endpoint=endpoint).observe(duration)
    return response


@app.get("/health")
def health(request: Request) -> dict[str, str]:
    """Health check endpoint. Returns service status and version."""
    trace_id = request.headers.get("x-trace-id", "")
    logger.info("health check", extra={"trace_id": trace_id})
    # Bump this version together with VERSION file at repo root
    return {"status": "ok", "service": "py-api", "version": PY_API_VERSION}


@app.get("/metrics")
def metrics() -> Response:
    """Prometheus metrics endpoint — returns raw metrics in OpenMetrics format."""
    return Response(
        content=generate_latest(),
        media_type=CONTENT_TYPE_LATEST,
    )
