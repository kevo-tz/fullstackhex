import logging
import os
from typing import Any, Optional

import redis.asyncio as redis

logger = logging.getLogger(__name__)

_redis_client: Optional[redis.Redis] = None


async def init_redis() -> redis.Redis:
    """Initialize and return a Redis client."""
    global _redis_client
    try:
        redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
        _redis_client = await redis.from_url(redis_url)
        await _redis_client.ping()
        logger.info("Redis connection established")
        return _redis_client
    except Exception as e:
        logger.error(f"Failed to initialize Redis: {e}")
        raise


async def close_redis() -> None:
    """Close the Redis connection."""
    global _redis_client
    if _redis_client:
        try:
            await _redis_client.close()
            logger.info("Redis connection closed")
        except Exception as e:
            logger.error(f"Error closing Redis connection: {e}")


def get_redis_client() -> Optional[redis.Redis]:
    """Get the current Redis client."""
    return _redis_client


async def get_from_cache(key: str) -> Optional[Any]:
    """
    Retrieve a value from the cache.

    Args:
        key: The cache key

    Returns:
        The cached value or None if not found
    """
    if not _redis_client:
        logger.warning("Redis client not initialized")
        return None

    try:
        value = await _redis_client.get(key)
        if value:
            logger.debug(f"Cache hit for key: {key}")
            return value.decode("utf-8") if isinstance(value, bytes) else value
        logger.debug(f"Cache miss for key: {key}")
        return None
    except Exception as e:
        logger.error(f"Error retrieving from cache: {e}")
        return None


async def set_in_cache(
    key: str, value: Any, ttl: Optional[int] = None
) -> bool:
    """
    Store a value in the cache.

    Args:
        key: The cache key
        value: The value to cache
        ttl: Time to live in seconds (optional)

    Returns:
        True if successful, False otherwise
    """
    if not _redis_client:
        logger.warning("Redis client not initialized")
        return False

    try:
        await _redis_client.set(key, value, ex=ttl)
        logger.debug(f"Value cached for key: {key}")
        return True
    except Exception as e:
        logger.error(f"Error setting cache: {e}")
        return False


async def delete_from_cache(key: str) -> bool:
    """
    Delete a value from the cache.

    Args:
        key: The cache key

    Returns:
        True if successful, False otherwise
    """
    if not _redis_client:
        logger.warning("Redis client not initialized")
        return False

    try:
        result = await _redis_client.delete(key)
        logger.debug(f"Deleted cache key: {key}")
        return result > 0
    except Exception as e:
        logger.error(f"Error deleting from cache: {e}")
        return False
