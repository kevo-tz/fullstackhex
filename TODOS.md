# TODO

## Deferred from v0.14

- [ ] **WebSocket auth token handshake** — `/live` is public. Real WS features (chat, notifications) need JWT during upgrade. See `docs/AUTH.md`.
- [x] **Cross-browser Playwright matrix** — Firefox/WebKit/Chromium all configured in playwright.config.ts.
- [x] **Nginx WS 101 validation test** — CI validates WS upgrade returns 101.
- [x] **Profile/settings page** — Implemented as profile.astro with auth guard.

## Known Items

- [x] **WS connection pooling on Rust side** — Now configurable via `WS_MAX_CONNECTIONS` env var with semaphore-based limiting.
- [ ] **Property test count low** — Only 3 proptest files. Add more invariant tests for domain types, rate limiting, session logic.
