# TODO

## [DONE] WsUserGuard: switch to std::sync::Mutex

**Commit:** `48a1443` — `backend/api/src/live.rs`, `backend/api/src/lib.rs`, 6 integration test files

`WsUserGuard::drop()` now uses `std::sync::Mutex::lock().unwrap()` instead of `tokio::sync::Mutex::try_lock()`. The per-user connection counter is always decremented.

---

## [DONE] Session deserialization: add old-format fallback

**Commit:** `876e302` — `backend/api/src/live.rs`

`cookie_authenticated()` now falls back to `cache_get::<String>` + JWT validation when the `Session` struct deserialization returns `None`. Old sessions migrate seamlessly.

---

## [P3] Remove JWT fallback after v0.13.7 deploy window

**File:** `backend/api/src/live.rs` — `cookie_authenticated()`

Once the deploy window closes (all sessions stored as `Session` structs), remove the fallback block. Marked in code with `// TODO: remove after v0.13.7 deploy window`.

**Target:** v0.13.9
