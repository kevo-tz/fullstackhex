from fastapi import FastAPI, Request
import logging
import json
import sys
import time

app = FastAPI()


class JsonFormatter(logging.Formatter):
    def format(self, record: logging.LogRecord) -> str:
        obj = {
            "timestamp": self.formatTime(record, "%Y-%m-%dT%H:%M:%S.%fZ"),
            "level": record.levelname.lower(),
            "target": record.name,
            "message": record.getMessage(),
        }
        if hasattr(record, "trace_id"):
            obj["trace_id"] = record.trace_id
        if record.exc_info and record.exc_info[1]:
            obj["error"] = str(record.exc_info[1])
        return json.dumps(obj)


def setup_logging() -> None:
    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(JsonFormatter())
    root = logging.getLogger()
    # Clear uvicorn handlers to avoid duplicate output
    root.handlers.clear()
    root.addHandler(handler)
    root.setLevel(logging.INFO)


setup_logging()
logger = logging.getLogger("python-sidecar")


@app.middleware("http")
async def trace_id_middleware(request: Request, call_next):
    trace_id = request.headers.get("x-trace-id", "")
    start = time.monotonic()
    response = await call_next(request)
    duration_ms = int((time.monotonic() - start) * 1000)
    logger.info(
        f"{request.method} {request.url.path} → {response.status_code}",
        extra={
            "trace_id": trace_id,
            "duration_ms": duration_ms,
            "status_code": response.status_code,
        },
    )
    return response


@app.get("/health")
def health(request: Request) -> dict[str, str]:
    trace_id = request.headers.get("x-trace-id", "")
    logger.info("health check", extra={"trace_id": trace_id})
    return {"status": "ok", "service": "python-sidecar", "version": "0.1.0"}
