# Contributing to FullStackHex

Thank you for contributing! Please read this guide before opening a PR.

## Branch Naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<short-description>` | `feat/add-tracing` |
| Bug fix | `fix/<short-description>` | `fix/socket-timeout` |
| Documentation | `docs/<short-description>` | `docs/update-setup` |
| Refactor | `refactor/<short-description>` | `refactor/crate-layout` |
| Infrastructure | `infra/<short-description>` | `infra/update-postgres` |

## PR Process

1. Fork the repo and create your branch from `main`.
2. Make your changes and ensure all checks pass locally (see below).
3. Open a pull request; fill in the PR template completely.
4. A maintainer will review within a reasonable time. Address feedback promptly.
5. PRs are squash-merged to keep history clean.

## Code Style

### Rust
```bash
cargo fmt --all          # Format
cargo clippy --all-targets --all-features -- -D warnings  # Lint
cargo test --workspace   # Tests
```

### Python
```bash
uv run ruff check .      # Lint
uv run ruff format .     # Format
uv run pytest            # Tests
```

### Frontend (Bun)
```bash
bun lint                 # Lint (requires prettier / eslint in project)
bun run test:vitest        # Tests (vitest)
bun run build            # Verify build succeeds
```

## Test Requirements

- Bug fixes must include a regression test where feasible.
- New features must include at least one test covering the happy path.
- All existing tests must pass before a PR will be reviewed.

## Commit Messages

Use conventional commit format:
```
feat: add unix socket reconnect logic
fix: resolve postgres healthcheck timeout
docs: clarify uv install step in SETUP.md
```

## Reporting Bugs

Use the bug report issue template. For security vulnerabilities, see [.github/SECURITY.md](.github/SECURITY.md) — do not open a public issue.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
