# Redis

Application-layer caching, session store, rate limiting, and pub/sub via Redis.

## Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `REDIS_URL` | — | Required. `redis://localhost:6379` |
| `REDIS_POOL_SIZE` | `10` | Connection pool size. |
| `REDIS_SAVE` | `"900 1 300 10 60 10000"` | Redis persistence schedule. Quote the value. |

Redis is optional — when `REDIS_URL` is unset, Redis-dependent features (rate limiting, sessions, caching) are disabled gracefully.

## Key Namespaces

All keys are prefixed to avoid collisions:

| Namespace | Pattern | TTL | Purpose |
|-----------|---------|-----|---------|
| `session` | `session:{id}` | Configurable | User session data. |
| `refresh` | `refresh:{token}` | `JWT_REFRESH_EXPIRY` | Refresh token → user_id mapping. |
| `blacklist` | `blacklist:{jti}` | `JWT_EXPIRY` | Blacklisted JWT IDs after logout. |
| `ratelimit` | `ratelimit:{key}` | Window + 60s | Sliding window rate limit counters. |
| `backoff` | `backoff:{ip}:{endpoint}` | Threshold-based | Brute-force failure counters. |
| `cache` | `cache:{key}` | Per-call | General-purpose key-value cache. |

## Session Store

Sessions are Redis-only — no PostgreSQL sessions table. Each session is a JSON blob:

```json
{
  "user_id": "...",
  "email": "...",
  "name": "...",
  "provider": "local|google|github",
  "created_at": 1700000000
}
```

Operations:
- `session_create(session, ttl)` — create session, returns session ID.
- `session_get(session_id)` — lookup session.
- `session_destroy(session_id)` — delete session.
- `session_refresh(session_id, ttl)` — extend TTL without changing data.

## Rate Limiting

Sliding window via Redis sorted sets. Each request adds a member with timestamp as score. Old entries cleaned on each check via Lua script (atomic).

## Token Refresh Atomicity

Refresh token rotation uses a Lua script for atomic GET+DEL — prevents token family leaks under concurrent refresh requests.

## Connection Pool Tuning

The pool uses `fred` with a configurable pool size. Increase `REDIS_POOL_SIZE` if you see pool timeout errors under load. Each `RedisClient` holds one connection pool shared across all operations.

## Pub/Sub

`cache/src/pubsub.rs` provides publish/subscribe helpers. Not currently wired into application routes — available for real-time features.
