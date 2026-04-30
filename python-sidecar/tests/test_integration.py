from fastapi.testclient import TestClient

from app.main import app


def test_sidecar_response_shape() -> None:
    client = TestClient(app)
    payload = client.get("/health").json()

    assert set(payload.keys()) == {"status", "service"}
    assert payload["service"] == "python-sidecar"
