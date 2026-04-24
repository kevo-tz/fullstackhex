import logging
import os
from contextlib import asynccontextmanager

from fastapi import FastAPI
from pydantic import BaseModel

logger = logging.getLogger(__name__)

DB_URL = os.getenv("PYTHON_SERVICE_DB_URL", "postgresql://user:password@localhost/bare_metal")


class HealthResponse(BaseModel):
    status: str


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting Python service...")
    logger.info(f"Database URL configured: {DB_URL}")
    yield
    logger.info("Shutting down Python service...")


app = FastAPI(title="Bare Metal Python Service", lifespan=lifespan)


@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint."""
    return HealthResponse(status="ok")


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8001)
