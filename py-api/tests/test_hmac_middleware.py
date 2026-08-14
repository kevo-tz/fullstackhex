"""Tests for HMAC authentication middleware.

Tests the middleware directly via async function calls to avoid adding
temporary routes to the shared app instance.
"""

import hashlib
import hmac
import json
import logging
import time

from fastapi import Request
from starlette.responses import Response

import pytest

from app.main import hmac_auth_middleware


def _make_request(
    path: str = "/test",
    headers: dict | None = None,
    method: str = "GET",
) -> Request:
    header_list = []
    for name, value in (headers or {}).items():
        header_list.append((name.lower().encode(), str(value).encode()))
    scope = {
        "type": "http",
        "method": method,
        "path": path,
        "query_string": b"",
        "headers": header_list,
    }
    return Request(scope)


async def _call_next_ok(request: Request) -> Response:
    return Response(content=b"ok", status_code=200)


def _valid_timestamp() -> str:
    return str(int(time.time()))


def _valid_signed_headers(secret: str, nonce: str) -> dict:
    ts = _valid_timestamp()
    payload = json.dumps(
        {
            "user_id": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "timestamp": int(ts),
        },
        sort_keys=True,
        separators=(",", ":"),
    )
    sig = hmac.new(
        secret.encode("utf-8"),
        payload.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()
    return {
        "X-User-Id": "user-123",
        "X-User-Email": "test@example.com",
        "X-User-Name": "Test User",
        "X-Auth-Signature": sig,
        "X-Timestamp": ts,
        "X-Nonce": nonce,
    }


class _BrokenRedis:
    async def set(self, *args, **kwargs):
        raise ConnectionError("Redis connection lost")


@pytest.mark.asyncio
async def test_hmac_missing_signature_returns_401():
    import app.main

    app.main.settings.shared_secret = "dummy_sidecar_secret"
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-Timestamp": _valid_timestamp(),
        }
    )
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Missing auth headers" in body["error"]


@pytest.mark.asyncio
async def test_hmac_invalid_signature_returns_401():
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
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Invalid auth signature" in body["error"]


@pytest.mark.asyncio
async def test_hmac_valid_signature_passes():
    secret = "dummy_sidecar_secret"
    import app.main

    app.main.settings.shared_secret = secret
    ts = _valid_timestamp()
    payload = json.dumps(
        {
            "user_id": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "timestamp": int(ts),
        },
        sort_keys=True,
        separators=(",", ":"),
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
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 200
    assert response.body == b"ok"


@pytest.mark.asyncio
async def test_hmac_expired_timestamp_returns_401():
    secret = "dummy_sidecar_secret"
    import app.main

    app.main.settings.shared_secret = secret
    ts = str(int(time.time()) - 60)
    payload = json.dumps(
        {
            "user_id": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "timestamp": int(ts),
        },
        sort_keys=True,
        separators=(",", ":"),
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
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Request expired" in body["error"]


@pytest.mark.asyncio
async def test_hmac_missing_timestamp_returns_401():
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
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Missing or invalid timestamp" in body["error"]


@pytest.mark.asyncio
async def test_hmac_missing_secret_rejects_all_requests():
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": "some-sig",
        }
    )
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "SIDECAR_SHARED_SECRET not configured" in body["error"]


@pytest.mark.asyncio
async def test_hmac_public_routes_skip_auth():
    import app.main

    app.main.settings.shared_secret = "dummy_sidecar_secret"
    for path in ("/health", "/metrics"):
        req = _make_request(path=path)
        response = await hmac_auth_middleware(req, _call_next_ok)
        assert response.status_code == 200, f"{path} should skip HMAC auth"


@pytest.mark.asyncio
async def test_hmac_redis_unavailable_fails_open_by_default(monkeypatch, caplog):
    import app.main

    secret = "dummy_sidecar_secret"
    app.main.settings.shared_secret = secret
    monkeypatch.setattr(app.main, "redis_client", None)
    monkeypatch.setattr(app.main.settings, "fail_open_on_redis_error", True)
    req = _make_request(method="POST", headers=_valid_signed_headers(secret, "nonce-123"))
    with caplog.at_level(logging.WARNING):
        response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 200
    assert response.body == b"ok"
    assert "skipping nonce dedup (fail-open)" in caplog.text


@pytest.mark.asyncio
async def test_hmac_redis_unavailable_fails_closed_when_configured(monkeypatch):
    import app.main

    secret = "dummy_sidecar_secret"
    app.main.settings.shared_secret = secret
    monkeypatch.setattr(app.main, "redis_client", None)
    monkeypatch.setattr(app.main.settings, "fail_open_on_redis_error", False)
    req = _make_request(method="POST", headers=_valid_signed_headers(secret, "nonce-123"))
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Auth service degraded" in body["error"]


@pytest.mark.asyncio
async def test_hmac_redis_error_fails_open_by_default(monkeypatch, caplog):
    import app.main

    secret = "dummy_sidecar_secret"
    app.main.settings.shared_secret = secret
    monkeypatch.setattr(app.main, "redis_client", _BrokenRedis())
    monkeypatch.setattr(app.main.settings, "fail_open_on_redis_error", True)
    req = _make_request(method="POST", headers=_valid_signed_headers(secret, "nonce-123"))
    with caplog.at_level(logging.WARNING):
        response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 200
    assert response.body == b"ok"
    assert "skipping nonce dedup (fail-open)" in caplog.text


@pytest.mark.asyncio
async def test_hmac_redis_error_fails_closed_when_configured(monkeypatch):
    import app.main

    secret = "dummy_sidecar_secret"
    app.main.settings.shared_secret = secret
    monkeypatch.setattr(app.main, "redis_client", _BrokenRedis())
    monkeypatch.setattr(app.main.settings, "fail_open_on_redis_error", False)
    req = _make_request(method="POST", headers=_valid_signed_headers(secret, "nonce-123"))
    response = await hmac_auth_middleware(req, _call_next_ok)
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Auth service degraded" in body["error"]
