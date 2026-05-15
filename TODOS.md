# TODO

## Deferred from v0.14

- [x] **WebSocket auth token handshake** — Already implemented: `live.rs:106-175` handles JWT query param, cookie auth, Origin validation, per-user quota, 401 on failure. Docs in `docs/AUTH.md#websocket-auth`.
- [x] **Cross-browser Playwright matrix** — Firefox/WebKit/Chromium all configured in playwright.config.ts.
- [x] **Nginx WS 101 validation test** — CI validates WS upgrade returns 101.
- [x] **Profile/settings page** — Implemented as profile.astro with auth guard.

## Known Items

- [x] **WS connection pooling on Rust side** — Now configurable via `WS_MAX_CONNECTIONS` env var with semaphore-based limiting.
- [x] **Property test count low** — 5 proptest files (domain, api, auth, cache, py-sidecar). All planned Phase 8 invariants covered. Lua script proptests deferred (requires Redis).
