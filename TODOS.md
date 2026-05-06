# TODOS

## Design (from /plan-design-review)

### D1. Dashboard empty state [P2] [S]
**What:** When all services are down, the dashboard shows 6 red dots with no recovery guidance.
**Fix:** Add an empty-state message with a "retry" button and a link to troubleshooting docs. Show it when all health statuses are "error".
**Files:** `frontend/src/pages/index.astro`

### D2. Mobile nav behavior [P2] [S]
**What:** Nav bar (brand + login link) may overlap on screens narrower than 320px. No hamburger or stacking behavior specified.
**Fix:** Stack nav items vertically on viewports < 400px, or use a hamburger menu. Brand stays visible, auth link moves below.
**Files:** `frontend/src/components/Layout.astro`

### D3. OAuth button behavior when unconfigured [P2] [S]
**What:** OAuth buttons (Google, GitHub) are always visible even when OAuth is not configured. Clicking leads to a 404 or error page.
**Fix:** Conditionally render OAuth buttons only when the respective provider is configured. Check via `/api/auth/oauth/{provider}` health or a config endpoint.
**Files:** `frontend/src/components/AuthForm.astro`

### D4. Dark mode toggle [P3] [S]
**What:** The theme is always dark. No light mode exists. Intentional for a dev tool template, but worth documenting.
**Fix:** Add a comment in DESIGN.md (once created) clarifying that dark-only is intentional. If light mode is desired later, implement CSS custom property toggle with `prefers-color-scheme` media query.
**Files:** `frontend/src/components/Layout.astro`, `docs/DESIGN.md` (to create)
