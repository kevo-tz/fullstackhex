import json
import logging

from fastapi.testclient import TestClient

from app.main import JsonFormatter, app


def test_health_endpoint() -> None:
    client = TestClient(app)
    response = client.get("/health")

    assert response.status_code == 200
    assert response.json()["status"] == "ok"


def test_health_with_trace_id_header() -> None:
    client = TestClient(app)
    response = client.get("/health", headers={"x-trace-id": "test-trace-abc"})

    assert response.status_code == 200
    assert response.json()["status"] == "ok"
    assert response.json()["service"] == "py-api"


def test_health_without_trace_id_header() -> None:
    client = TestClient(app)
    response = client.get("/health")

    assert response.status_code == 200
    assert response.json()["status"] == "ok"


def test_json_formatter_includes_trace_id() -> None:
    formatter = JsonFormatter()
    record = logging.makeLogRecord(
        {
            "name": "test-logger",
            "levelno": logging.INFO,
            "levelname": "INFO",
            "msg": "test message",
            "args": (),
            "trace_id": "abc-123",
        }
    )
    output = formatter.format(record)
    parsed = json.loads(output)

    assert parsed["trace_id"] == "abc-123"
    assert parsed["level"] == "info"
    assert parsed["target"] == "test-logger"
    assert parsed["message"] == "test message"


def test_json_formatter_without_trace_id() -> None:
    formatter = JsonFormatter()
    record = logging.makeLogRecord(
        {
            "name": "no-trace",
            "levelno": logging.INFO,
            "levelname": "INFO",
            "msg": "no trace here",
            "args": (),
        }
    )
    output = formatter.format(record)
    parsed = json.loads(output)

    assert "trace_id" not in parsed
    assert parsed["level"] == "info"
    assert parsed["message"] == "no trace here"


def test_json_formatter_includes_error_on_exception() -> None:
    import sys

    formatter = JsonFormatter()
    try:
        raise RuntimeError("something broke")
    except RuntimeError:
        record = logging.makeLogRecord(
            {
                "name": "err-logger",
                "levelno": logging.ERROR,
                "levelname": "ERROR",
                "msg": "an error occurred",
                "args": (),
                "exc_info": sys.exc_info(),
            }
        )
    output = formatter.format(record)
    parsed = json.loads(output)

    assert parsed["level"] == "error"
    assert parsed["message"] == "an error occurred"
    assert "error" in parsed
    assert parsed["error"] == "something broke"


def test_json_formatter_timestamp_format() -> None:
    formatter = JsonFormatter()
    record = logging.makeLogRecord(
        {
            "name": "ts-test",
            "levelno": logging.INFO,
            "levelname": "INFO",
            "msg": "timestamp check",
            "args": (),
        }
    )
    output = formatter.format(record)
    parsed = json.loads(output)

    assert parsed["timestamp"].endswith("Z")
    # Basic ISO-8601 format check: YYYY-MM-DDTHH:MM:SS.ffffffZ
    assert parsed["timestamp"][10] == "T"
    assert "." in parsed["timestamp"]  # has fractional seconds
