"""Tests for HMAC authentication middleware.

Tests the middleware directly via async function calls to avoid adding
temporary routes to the shared app instance.
"""

import asyncio
import hashlib
import hmac
import json
import time

from fastapi import Request
from starlette.responses import Response

from app.main import hmac_auth_middleware


def _make_request(
    path: str = "/test",
    headers: dict | None = None,
) -> Request:
    header_list = []
    for name, value in (headers or {}).items():
        header_list.append((name.lower().encode(), str(value).encode()))
    scope = {
        "type": "http",
        "method": "GET",
        "path": path,
        "query_string": b"",
        "headers": header_list,
    }
    return Request(scope)


async def _call_next_ok(request: Request) -> Response:
    return Response(content=b"ok", status_code=200)


def _valid_timestamp() -> str:
    return str(int(time.time()))


def test_hmac_missing_signature_returns_401():
    import app.main

    app.main.settings.shared_secret = "dummy_sidecar_secret"
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-Timestamp": _valid_timestamp(),
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Missing auth headers" in body["error"]


def test_hmac_invalid_signature_returns_401():
    import app.main

    app.main.settings.shared_secret = "dummy_sidecar_secret"
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": "invalid-signature",
            "X-Timestamp": _valid_timestamp(),
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Invalid auth signature" in body["error"]


def test_hmac_valid_signature_passes():
    secret = "dummy_sidecar_secret"
    import app.main

    app.main.settings.shared_secret = secret
    ts = _valid_timestamp()
    # Use JSON-based HMAC payload matching production middleware
    payload = json.dumps(
        {
            "user_id": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "timestamp": int(ts),
        },
        sort_keys=True,
    )
    sig = hmac.new(
        secret.encode("utf-8"),
        payload.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": sig,
            "X-Timestamp": ts,
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 200
    assert response.body == b"ok"


def test_hmac_expired_timestamp_returns_401():
    secret = "dummy_sidecar_secret"
    import app.main

    app.main.settings.shared_secret = secret
    # Timestamp 60s in the past (outside ±30s window)
    ts = str(int(time.time()) - 60)
    payload = json.dumps(
        {
            "user_id": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "timestamp": int(ts),
        },
        sort_keys=True,
    )
    sig = hmac.new(
        secret.encode("utf-8"),
        payload.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": sig,
            "X-Timestamp": ts,
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Request expired" in body["error"]


def test_hmac_missing_timestamp_returns_401():
    import app.main

    app.main.settings.shared_secret = "dummy_sidecar_secret"
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": "some-sig",
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Missing or invalid timestamp" in body["error"]


def test_hmac_missing_secret_rejects_all_requests():
    # SIDECAR_SHARED_SECRET is removed by the fixture
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": "some-sig",
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "SIDECAR_SHARED_SECRET not configured" in body["error"]


def test_hmac_public_routes_skip_auth():
    import app.main

    app.main.settings.shared_secret = "dummy_sidecar_secret"
    for path in ("/health", "/metrics"):
        req = _make_request(path=path)
        response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
        assert response.status_code == 200, f"{path} should skip HMAC auth"
