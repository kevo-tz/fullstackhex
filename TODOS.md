# TODO

## Deferred from v0.14

- [ ] **WebSocket auth token handshake** — `/live` is public. Real WS features (chat, notifications) need JWT during upgrade. See `docs/AUTH.md`.
- [ ] **Cross-browser Playwright matrix** — Chromium-only now. Add Firefox/WebKit when e2e suite stabilizes.
- [ ] **Nginx WS 101 validation test** — CI should explicitly test `/api/live` returns 101 Switching Protocols when Redis is available.
- [ ] **Profile/settings page** — Removed from v0.14 scope. CRUD pages (notes) cover form patterns already.

## Known Items

- [ ] **WS connection pooling on Rust side** — `MAX_WS_CONNECTIONS=100` is hardcoded. Make configurable via env var.
- [ ] **Property test count low** — Only 3 proptest files. Add more invariant tests for domain types, rate limiting, session logic.
- [ ] **Pagination on notes list** — `Pagination.astro` exists but backend notes endpoint lacks pagination params. Currently returns all notes.
