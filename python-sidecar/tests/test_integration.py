"""
Live integration tests — require the Python sidecar to be running on the
configured Unix socket. Run with:

    uv run pytest -m integration

These are skipped in normal test runs.
"""

import os

import pytest
import httpx


@pytest.mark.integration
def test_live_health_endpoint() -> None:
    socket_path = os.environ.get("PYTHON_SIDECAR_SOCKET", "/tmp/python-sidecar.sock")
    transport = httpx.HTTPTransport(uds=socket_path)
    with httpx.Client(transport=transport, base_url="http://localhost") as client:
        response = client.get("/health")

    assert response.status_code == 200
    payload = response.json()
    assert set(payload.keys()) >= {"status", "service", "version"}
    assert payload["status"] == "ok"
    assert payload["service"] == "python-sidecar"
