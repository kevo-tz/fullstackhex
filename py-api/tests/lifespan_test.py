"""Tests for the FastAPI lifespan and startup initialization.

Covers:
- lifespan context manager calls setup_logging without error
- _register_metrics creates module-level counters at import
- _register_metrics is idempotent (second call is a no-op)
- Settings behavior with environment variable
- Health endpoint metadata
"""

import logging
from importlib.metadata import version

from app.main import register_metrics, lifespan, app

PY_API_VERSION = version("py-api")


def test_register_metrics_creates_counters() -> None:
    """Calling register_metrics explicitly creates module-level counters."""
    register_metrics()
    from app.main import PYTHON_REQUESTS_TOTAL, PYTHON_REQUEST_DURATION

    assert PYTHON_REQUESTS_TOTAL._name is not None
    assert PYTHON_REQUEST_DURATION._name is not None


def test_register_metrics_idempotent() -> None:
    """Second call to register_metrics does not raise (idempotency guard)."""
    register_metrics()  # first call — may or may not register depending on import order
    register_metrics()  # second call — must be no-op, must not raise


def test_lifespan_calls_setup_logging() -> None:
    """Entering the lifespan context should add a StreamHandler to root logger."""
    import sys

    root = logging.getLogger()
    stderr_before = [
        h for h in root.handlers if isinstance(h, logging.StreamHandler) and h.stream is sys.stderr
    ]

    async def run():
        async with lifespan(app):
            pass

    import asyncio

    asyncio.run(run())

    # setup_logging adds a stderr StreamHandler; guard-inspect only stderr handlers
    # to avoid false negatives from pytest FileHandler (also a StreamHandler subclass).
    stderr_after = [
        h for h in root.handlers if isinstance(h, logging.StreamHandler) and h.stream is sys.stderr
    ]
    assert len(stderr_after) == len(stderr_before) + 1


def test_settings_uses_env_var(monkeypatch) -> None:
    """Settings should read SIDECAR_SHARED_SECRET from environment."""
    monkeypatch.setenv("SIDECAR_SHARED_SECRET", "test-secret-value")
    from app.main import Settings

    s = Settings()
    assert s.shared_secret == "test-secret-value"


def test_health_endpoint_version() -> None:
    """Health endpoint should return version matching py-api."""
    from fastapi.testclient import TestClient

    client = TestClient(app)
    response = client.get("/health")
    assert response.status_code == 200
    data = response.json()
    assert data["version"] == PY_API_VERSION
    assert data["service"] == "py-api"
