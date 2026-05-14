import pytest
import logging
import os
import sys


@pytest.fixture(autouse=True)
def _clean_env(monkeypatch):
    """Remove SIDECAR_SHARED_SECRET before each test and reset module-level cache."""
    monkeypatch.delenv("SIDECAR_SHARED_SECRET", raising=False)
    import app.main

    app.main.settings.shared_secret = ""
    yield
    app.main.settings.shared_secret = os.environ.get("SIDECAR_SHARED_SECRET", "")
    # Clean up StreamHandler(s) added by setup_logging to prevent test pollution
    root = logging.getLogger()
    for h in list(root.handlers):
        if isinstance(h, logging.StreamHandler) and h.stream is sys.stderr:
            root.removeHandler(h)
