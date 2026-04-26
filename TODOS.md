# FullStackHex Open Source Launch Plan
*Post-Rename Public Repo Readiness Plan*

---

## Phase 3: Push to Public GitHub Repo
1. Create public GitHub repo named `fullstackhex`
2. Verify: `git status` shows no `.env` file tracked
3. Push local repo to GitHub
4. Add repo metadata: description, tags (`rust`, `python`, `sidecar`, `full-stack`, `template`), website
5. Enable GitHub Discussions and Issues

---

## Phase 4: P1 Post-Release Tasks (Complete within 1 week of going public)

| # | File/Action | Description | Priority |
|---|-------------|-------------|----------|
| 1 | `.github/workflows/ci.yml` | CI pipeline: Rust `cargo fmt`/`clippy`/`test`, Python `ruff`/`pytest`, frontend `bun lint`/`bun test` | 🟡 P1 |
| 2 | `Dockerfile.rust` | Multi-stage Rust backend build: compile release binary, minimal runtime image | 🟡 P1 |
| 3 | `Dockerfile.python` | Python sidecar build: uv install, FastAPI/Uvicorn dependencies | 🟡 P1 |
| 4 | `Dockerfile.frontend` | Astro frontend build: bun install, bun run build, static file serving | 🟡 P1 |
| 5 | `docker-compose.prod.yml` | Production config: no optional tools (adminer), resource limits, no default passwords, Nginx service | 🟡 P1 |
| 6 | `nginx/nginx.conf` | Reverse proxy: ports 80/443, security headers, proxy to frontend (:4321) and backend (:8001), TLS config placeholder | 🟡 P1 |
| 7 | `VERSION` | Initial version `0.1.0`, follow SemVer | 🟡 P1 |
| 8 | `CHANGELOG.md` | Initial entry: "0.1.0 - Initial open source release" | 🟡 P1 |

---

## Phase 5: P2 Long-Term Tasks (1-3 months post-launch)
1. Add `.github/FUNDING.yml` for sponsorship
2. Add `docker-compose.monitor.yml` with Prometheus + Grafana stack
3. Add test suites for generated Rust/Python/Frontend code
4. Add `.env.prod.example` separate production env template
5. Set up dependabot for dependency updates
6. Add pre-commit hook or CI step for secret scanning (`gitleaks` or `detect-secrets`)

---

## Final Pre-Push Validation Checklist
- [ ] `grep -RInE "app_pass|devadmin" --exclude-dir=.git .` returns zero hits
- [ ] `grep -nE '\$\{[A-Z_]+:-[a-z]' docker-compose*.yml` returns only non-sensitive defaults (ports, Redis tuning)
- [ ] No `.env` file committed (`git ls-files .env` returns empty)
- [ ] `.gitignore` covers `.env`, `.env.local`, `.env.*.local`
- [ ] `LICENSE` file present and correct (MIT, 2026)
- [ ] `README.md` clearly explains project purpose, components, and quick start
- [ ] `.github/SECURITY.md` present with contact/disclosure policy
- [ ] `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md` present
- [ ] Issue templates and PR template present in `.github/`
- [ ] GitHub repo created and verified available

