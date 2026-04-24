import logging
import os
from contextlib import asynccontextmanager

from fastapi import FastAPI
from pydantic import BaseModel

from cache import close_redis, get_from_cache, init_redis, set_in_cache

logger = logging.getLogger(__name__)

DB_URL = os.getenv("DATABASE_URL", "postgresql://user:password@localhost/bare_metal")


class HealthResponse(BaseModel):
    status: str


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting Python service...")
    logger.info(f"Database URL configured: {DB_URL}")
    await init_redis()
    yield
    await close_redis()
    logger.info("Shutting down Python service...")


app = FastAPI(title="Bare Metal Python Service", lifespan=lifespan)


@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint."""
    return HealthResponse(status="ok")


@app.get("/cache-test")
async def cache_test():
    """Example endpoint demonstrating cache usage."""
    cache_key = "test_key"

    # Try to get from cache
    cached_value = await get_from_cache(cache_key)
    if cached_value:
        return {"message": "Value retrieved from cache", "value": cached_value}

    # If not in cache, compute value and store it
    computed_value = "Hello from Python service"
    await set_in_cache(cache_key, computed_value, ttl=3600)
    return {"message": "Value computed and cached", "value": computed_value}


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8001)
