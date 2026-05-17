# TODO

## [P1] WsUserGuard: switch to std::sync::Mutex

**File:** `backend/api/src/live.rs`

**Problem:** `WsUserGuard::drop()` uses `try_lock()` on a `tokio::sync::Mutex`. If the lock is contended (unlikely but possible), the per-user connection counter silently isn't decremented, slowly inflating counts.

**Fix:**
- Change `ws_user_connections` type from `Arc<tokio::sync::Mutex<HashMap<String, usize>>>` to `Arc<std::sync::Mutex<HashMap<String, usize>>>`
- Update `WsUserGuard::drop()` to use `.lock().unwrap()` instead of `.try_lock()`
- `std::sync::Mutex` is fine here — the lock is held for nanoseconds, and `drop()` already runs in a blocking context (the WS recv task)

**Files to touch:**
- `backend/api/src/live.rs` — type change + lock call
- Any other file referencing `ws_user_connections` type

---

## [P1] Session deserialization: add old-format fallback

**File:** `backend/api/src/live.rs` — `cookie_authenticated()`

**Problem:** Sessions stored in Redis before v0.13.7 are raw JWT strings, but the code now expects `Session` struct. Old sessions deserialize as `None`, forcing re-authentication after deploy.

**Fix:**
- After `cache_get::<Session>` returns `None`, fall back to `cache_get::<String>` for the same key
- If the old format exists, validate the JWT via `auth_service.jwt.validate_token()` and extract `sub`
- Add a `TODO: remove after v0.13.7 deploy window` comment
- If `auth_service` is not available (None), don't fall back

**Note:** `cookie_authenticated` currently takes `&AppState` but needs access to `auth_service`. Either pass `auth_service` directly or access via `state.auth`.

---

## Deployment notes
- Deploy the session fallback first, then the WsUserGuard fix — no special ordering required
- Remove session fallback in v0.13.9
