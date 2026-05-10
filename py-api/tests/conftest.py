import pytest


@pytest.fixture(autouse=True)
def _clean_env(monkeypatch):
    """Remove SIDECAR_SHARED_SECRET before each test."""
    monkeypatch.delenv("SIDECAR_SHARED_SECRET", raising=False)