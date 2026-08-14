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
    redis_client = aioredis.from_url(
        redis_url,
        decode_responses=True,
        max_connections=10,
        socket_timeout=0.5,
        socket_connect_timeout=0.5,
        health_check_interval=30,
    )
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


def _env_bool(name: str, default: bool) -> bool:
    """Parse a truthy/falsy env var, mirroring the Rust side's env_bool."""
    value = os.environ.get(name)
    if value is None or value.strip() == "":
        return default
    return value.strip().lower() in ("1", "true", "yes", "on")


class Settings:
    """Application settings, sourced from environment at startup."""

    def __init__(self) -> None:
        secret = os.environ.get("SIDECAR_SHARED_SECRET", "")
        if not secret:
            logging.warning("SIDECAR_SHARED_SECRET is empty — all requests will be rejected")
        self.shared_secret: str = secret
        self.fail_open_on_redis_error: bool = _env_bool(
            "SIDECAR_FAIL_OPEN_ON_REDIS_ERROR", default=True
        )


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


_UUID_PATTERN = re.compile(r"/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
_NUMERIC_PATTERN = re.compile(r"/\d+")


def _normalize_endpoint(path: str) -> str:
    """Replace UUID and numeric segments with `{id}` to prevent Prometheus label cardinality explosion."""
    normalized = _UUID_PATTERN.sub("/{id}", path)
    normalized = _NUMERIC_PATTERN.sub("/{id}", normalized)
    dynamic_segments = [s for s in normalized.split("/") if s.startswith("{")]
    if len(dynamic_segments) > 2:
        return "unknown"
    return normalized


def _reject(
    log_message: str, status_message: str, trace_id: str, extra: dict | None = None
) -> Response:
    """Log an HMAC rejection and build the 401 JSON response."""
    logger.warning(
        f"HMAC rejection: {log_message}",
        extra={**(extra or {}), "trace_id": trace_id},
    )
    return Response(
        content=json.dumps({"error": status_message}),
        status_code=401,
        media_type="application/json",
    )


async def _validate_hmac(headers: dict[str, str], method: str, trace_id: str) -> Response | None:
    """Validate HMAC-SHA256 auth headers. Returns a 401 Response or None to allow.

    Order matters:
      1. timestamp validation
      2. payload build + HMAC verify (signature checked BEFORE the nonce replay
         check so invalid signatures never write 90s-TTL Redis keys)
      3. nonce SET NX — only for state-changing methods (POST/PUT/DELETE/PATCH);
         replay of an idempotent GET/HEAD/OPTIONS is harmless and skipped.
    """
    if not settings.shared_secret:
        return _reject(
            "SIDECAR_SHARED_SECRET not configured",
            "SIDECAR_SHARED_SECRET not configured — rejecting all requests",
            trace_id,
        )

    user_id = headers.get("x-user-id", "")
    email = headers.get("x-user-email", "")
    name = headers.get("x-user-name", "")
    signature = headers.get("x-auth-signature", "")
    timestamp_str = headers.get("x-timestamp", "")
    nonce = headers.get("x-nonce", "")

    if not all([user_id, email, signature]):
        return _reject(
            "missing auth headers",
            "Missing auth headers",
            trace_id,
            extra={"has_user_id": bool(user_id), "has_email": bool(email)},
        )

    # Validate timestamp (±30s window)
    try:
        ts = int(timestamp_str)
        now = int(time.time())
        if abs(now - ts) > 30:
            return _reject(
                "timestamp outside window",
                "Request expired",
                trace_id,
                extra={"timestamp": ts, "skew": now - ts},
            )
    except ValueError, TypeError:
        return _reject(
            "missing or invalid timestamp",
            "Missing or invalid timestamp",
            trace_id,
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
        return _reject(
            "invalid signature",
            "Invalid auth signature",
            trace_id,
            extra={"user_id": user_id, "email": email},
        )

    # Replay protection: check nonce hasn't been seen (atomic SET NX).
    # Signature already verified above, so this can only be triggered by a
    # legitimately-signed request; skip for idempotent methods.
    if nonce and method in ("POST", "PUT", "DELETE", "PATCH"):
        if redis_client is None:
            if settings.fail_open_on_redis_error:
                logger.warning(
                    "HMAC: Redis unavailable — skipping nonce dedup (fail-open)",
                    extra={"trace_id": trace_id},
                )
            else:
                return _reject(
                    "nonce provided but Redis unavailable — rejecting",
                    "Auth service degraded",
                    trace_id,
                )
        else:
            nonce_key = f"hmac:nonce:{nonce}"
            try:
                set_ok = await redis_client.set(nonce_key, "1", nx=True, ex=90)
            except Exception as e:
                if settings.fail_open_on_redis_error:
                    logger.warning(
                        "HMAC: Redis error — skipping nonce dedup (fail-open): %s",
                        e,
                        extra={"trace_id": trace_id},
                    )
                else:
                    return _reject(
                        "nonce provided but Redis errored — rejecting",
                        "Auth service degraded",
                        trace_id,
                        extra={"error": str(e)},
                    )
            else:
                if not set_ok:
                    return _reject(
                        "duplicate nonce (replay)",
                        "Duplicate request",
                        trace_id,
                        extra={"nonce": nonce},
                    )

    return None


async def hmac_auth_middleware(
    request: Request, call_next: Callable[[Request], Awaitable[Response]]
) -> Response:
    """Compatibility shim over _validate_hmac kept for direct test invocation."""
    trace_id = request.headers.get("x-trace-id", "")
    path = request.url.path
    if path in ("/health", "/metrics"):
        return await call_next(request)
    headers = {
        k.decode("latin-1").lower(): v.decode("latin-1")
        for k, v in request.scope.get("headers", [])
    }
    response = await _validate_hmac(headers, request.method, trace_id)
    if response is not None:
        return response
    return await call_next(request)


class CombinedMiddleware:
    """Pure-ASGI middleware merging HMAC auth validation with trace logging + metrics.

    Replaces the two stacked BaseHTTPMiddleware with a single ASGI middleware,
    halving per-request overhead while preserving exact behavior.
    """

    def __init__(self, app):
        self.app = app

    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            return await self.app(scope, receive, send)

        method = scope.get("method", "GET")
        path = scope.get("path", "/")
        headers = {
            k.decode("latin-1").lower(): v.decode("latin-1") for k, v in scope.get("headers", [])
        }
        trace_id = headers.get("x-trace-id", "")
        is_public = path in ("/health", "/metrics")

        start = time.monotonic()

        # (a) HMAC auth validation — skipped for public paths
        if not is_public:
            rejection = await _validate_hmac(headers, method, trace_id)
            if rejection is not None:
                self._record(path, method, trace_id, 401, time.monotonic() - start)
                body = rejection.body
                await send(
                    {
                        "type": "http.response.start",
                        "status": 401,
                        "headers": [
                            (b"content-type", b"application/json"),
                            (b"content-length", str(len(body)).encode()),
                        ],
                    }
                )
                await send({"type": "http.response.body", "body": body})
                return

        # (b) Time the request, call downstream, log + record metrics
        status_code = {"value": 200}

        async def _send(message):
            if message["type"] == "http.response.start":
                status_code["value"] = message["status"]
            await send(message)

        await self.app(scope, receive, _send)
        self._record(path, method, trace_id, status_code["value"], time.monotonic() - start)

    @staticmethod
    def _record(path, method, trace_id, status_code, duration):
        duration_ms = int(duration * 1000)
        logger.info(
            f"{method} {path} → {status_code}",
            extra={
                "trace_id": trace_id,
                "duration_ms": duration_ms,
                "status_code": status_code,
            },
        )
        if path in ("/health", "/metrics"):
            return
        endpoint = _normalize_endpoint(path)
        status = str(status_code)
        PYTHON_REQUESTS_TOTAL.labels(method=method, endpoint=endpoint, status=status).inc()
        PYTHON_REQUEST_DURATION.labels(method=method, endpoint=endpoint).observe(duration)


app.add_middleware(CombinedMiddleware)


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
