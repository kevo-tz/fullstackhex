#!/bin/bash
# FullStackHex Python Sidecar Setup
# Scaffold the Python sidecar service

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

log_info "Scaffolding Python sidecar..."

# Create python-sidecar directory
mkdir -p python-sidecar
pushd python-sidecar > /dev/null

# Create pyproject.toml if it doesn't exist
if [ ! -f pyproject.toml ]; then
    cat > pyproject.toml << 'EOF'
[project]
name = "python-sidecar"
version = "0.1.0"
description = "FullStackHex Python sidecar"
requires-python = ">=3.14"
dependencies = [
  "fastapi>=0.116.0",
  "uvicorn>=0.35.0"
]

[dependency-groups]
dev = [
  "pytest>=8.4.0",
  "ruff>=0.8.0",
  "httpx>=0.28.0"
]

[tool.pytest.ini_options]
testpaths = ["tests"]
pythonpath = ["."]

[tool.ruff]
line-length = 100
target-version = "py314"
EOF
    log_success "Created python-sidecar/pyproject.toml"
fi

# Create directory structure
mkdir -p app tests

# Create app/__init__.py if it doesn't exist
if [ ! -f app/__init__.py ]; then
    cat > app/__init__.py << 'EOF'
# Generated package marker for Python sidecar app module.
EOF
    log_success "Created app/__init__.py"
fi

# Create app/main.py if it doesn't exist
if [ ! -f app/main.py ]; then
    cat > app/main.py << 'EOF'
from fastapi import FastAPI

app = FastAPI()


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok", "service": "python-sidecar"}
EOF
    log_success "Created python sidecar app entrypoint"
fi

# Create tests/test_health.py if it doesn't exist
if [ ! -f tests/test_health.py ]; then
    cat > tests/test_health.py << 'EOF'
from fastapi.testclient import TestClient

from app.main import app


def test_health_endpoint() -> None:
    client = TestClient(app)
    response = client.get("/health")

    assert response.status_code == 200
    assert response.json()["status"] == "ok"
EOF
    log_success "Created python unit test"
fi

# Create tests/test_integration.py if it doesn't exist
if [ ! -f tests/test_integration.py ]; then
    cat > tests/test_integration.py << 'EOF'
from fastapi.testclient import TestClient

from app.main import app


def test_sidecar_response_shape() -> None:
    client = TestClient(app)
    payload = client.get("/health").json()

    assert set(payload.keys()) == {"status", "service"}
    assert payload["service"] == "python-sidecar"
EOF
    log_success "Created python integration test"
fi

popd > /dev/null
log_success "Python sidecar scaffolding completed"
exit 0