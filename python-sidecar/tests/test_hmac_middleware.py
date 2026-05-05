"""Tests for HMAC authentication middleware.

Tests the middleware directly via async function calls to avoid adding
temporary routes to the shared app instance.
"""
import asyncio
import hashlib
import hmac
import json

import pytest
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


def _valid_signature(secret: str, user_id: str, email: str, name: str) -> str:
    payload = f"{user_id}|{email}|{name}"
    return hmac.new(
        secret.encode("utf-8"),
        payload.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()


@pytest.fixture(autouse=True)
def _clean_env(monkeypatch):
    """Remove SIDECAR_SHARED_SECRET before each test."""
    monkeypatch.delenv("SIDECAR_SHARED_SECRET", raising=False)


async def _call_next_ok(request: Request) -> Response:
    return Response(content=b"ok", status_code=200)


def test_hmac_missing_signature_returns_401(monkeypatch):
    monkeypatch.setenv("SIDECAR_SHARED_SECRET", "test-secret")
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Missing auth headers" in body["error"]


def test_hmac_invalid_signature_returns_401(monkeypatch):
    monkeypatch.setenv("SIDECAR_SHARED_SECRET", "test-secret")
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": "invalid-signature",
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 401
    body = json.loads(response.body)
    assert "Invalid auth signature" in body["error"]


def test_hmac_valid_signature_passes(monkeypatch):
    secret = "test-secret"
    monkeypatch.setenv("SIDECAR_SHARED_SECRET", secret)
    sig = _valid_signature(secret, "user-123", "test@example.com", "Test User")
    req = _make_request(
        headers={
            "X-User-Id": "user-123",
            "X-User-Email": "test@example.com",
            "X-User-Name": "Test User",
            "X-Auth-Signature": sig,
        }
    )
    response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
    assert response.status_code == 200
    assert response.body == b"ok"


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


def test_hmac_public_routes_skip_auth(monkeypatch):
    monkeypatch.setenv("SIDECAR_SHARED_SECRET", "test-secret")
    for path in ("/health", "/metrics"):
        req = _make_request(path=path)
        response = asyncio.run(hmac_auth_middleware(req, _call_next_ok))
        assert response.status_code == 200, f"{path} should skip HMAC auth"
