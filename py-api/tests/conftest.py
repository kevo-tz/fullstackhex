import pytest
import os


@pytest.fixture(autouse=True)
def _clean_env(monkeypatch):
    """Remove SIDECAR_SHARED_SECRET before each test and reset module-level cache."""
    monkeypatch.delenv("SIDECAR_SHARED_SECRET", raising=False)
    import app.main

    app.main.settings.shared_secret = ""
    yield
    app.main.settings.shared_secret = os.environ.get("SIDECAR_SHARED_SECRET", "")
