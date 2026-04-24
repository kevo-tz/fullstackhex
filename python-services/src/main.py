import logging
import os
from contextlib import asynccontextmanager

from fastapi import FastAPI
from prometheus_fastapi_instrumentator import Instrumentator
from pydantic import BaseModel

from cache import close_redis, get_from_cache, init_redis, set_in_cache

logger = logging.getLogger(__name__)

DB_URL = os.getenv("DATABASE_URL", "postgresql://user:password@localhost/bare_metal")


class HealthResponse(BaseModel):
    status: str
    database: bool = False
    redis: bool = False


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting Python service...")
    logger.info(f"Database URL configured: {DB_URL}")
    
    # Initialize Redis
    await init_redis()
    
    yield
    
    await close_redis()
    logger.info("Shutting down Python service...")


app = FastAPI(title="Bare Metal Python Service", lifespan=lifespan)

# Prometheus metrics
Instrumentator().instrument(app).expose(app)


@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint with DB and Redis checks."""
    from psycopg import AsyncConnection
    
    db_ok = False
    try:
        conn = await AsyncConnection.connect(DB_URL)
        await conn.execute("SELECT 1")
        await conn.close()
        db_ok = True
    except Exception as e:
        logger.error(f"DB health check failed: {e}")
    
    redis_ok = False
    try:
        from cache import _redis_client
        if _redis_client:
            await _redis_client.ping()
            redis_ok = True
    except Exception as e:
        logger.error(f"Redis health check failed: {e}")
    
    return HealthResponse(
        status="ok" if db_ok and redis_ok else "degraded",
        database=db_ok,
        redis=redis_ok
    )


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
    
    # Use uvloop for better performance
    uvicorn.run(
        app,
        host="0.0.0.0",
        port=8000,
        loop="uvloop" if os.name != "nt" else "asyncio",
        http="httptools" if os.name != "nt" else "h11"
    )
