# TODO

## [DONE] WsUserGuard: switch to std::sync::Mutex

**Commit:** `48a1443` — `backend/api/src/live.rs`, `backend/api/src/lib.rs`, 6 integration test files

`WsUserGuard::drop()` now uses `std::sync::Mutex::lock().unwrap()` instead of `tokio::sync::Mutex::try_lock()`. The per-user connection counter is always decremented.

---

## [DONE] Session deserialization: add old-format fallback

**Commit:** `876e302` — `backend/api/src/live.rs`

`cookie_authenticated()` now falls back to `cache_get::<String>` + JWT validation when the `Session` struct deserialization returns `None`. Old sessions migrate seamlessly.

---

## [DONE] Remove JWT fallback after v0.13.7 deploy window

**Commit:** _(pending)_ — `backend/api/src/live.rs`

The fallback block in `cookie_authenticated()` that read old-format JWT sessions has been removed. All sessions are now stored as `Session` structs.
