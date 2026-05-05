# Auth

Authentication via JWT access tokens + refresh token rotation + OAuth2 login.

## Quick Start

```bash
# Generate a secure secret
openssl rand -hex 32

# Add to .env
JWT_SECRET=<your-secret>
```

Auth is disabled when `JWT_SECRET` is unset or `CHANGE_ME`. The health dashboard shows auth status.

## Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `JWT_SECRET` | — | Required. 64-char hex recommended. |
| `JWT_ISSUER` | `fullstackhex` | Issuer claim in tokens. |
| `JWT_EXPIRY` | `900` | Access token lifetime in seconds (15 min). |
| `JWT_REFRESH_EXPIRY` | `604800` | Refresh token lifetime (7 days). |
| `AUTH_MODE` | `both` | `bearer`, `cookie`, or `both`. Bearer takes precedence in `both` mode. |

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/auth/register` | None | Create user with email/password. |
| POST | `/auth/login` | None | Login, returns JWT + refresh token. |
| POST | `/auth/logout` | Required | Blacklists JTI, destroys session. |
| POST | `/auth/refresh` | None | Exchange refresh token for new JWT. |
| GET | `/auth/me` | Required | Return current user info. |
| GET | `/auth/oauth/{provider}` | None | Start OAuth login (google, github). |
| GET | `/auth/oauth/{provider}/callback` | None | OAuth callback. |

## OAuth Providers

### Google

1. Create OAuth 2.0 credentials in [Google Cloud Console](https://console.cloud.google.com/apis/credentials).
2. Set redirect URI: `http://localhost:8001/auth/oauth/google/callback`.
3. Add to `.env`:
   ```
   GOOGLE_CLIENT_ID=your-client-id
   GOOGLE_CLIENT_SECRET=your-client-secret
   ```

### GitHub

1. Create OAuth App in [GitHub Developer Settings](https://github.com/settings/developers).
2. Set callback URL: `http://localhost:8001/auth/oauth/github/callback`.
3. Add to `.env`:
   ```
   GITHUB_CLIENT_ID=your-client-id
   GITHUB_CLIENT_SECRET=your-client-secret
   ```

## Brute-Force Protection

Progressive backoff on login failures per IP:

- 5 failures → 60s block
- 10 failures → 5min block
- 20 failures → 30min block

Backoff is checked before the rate limit. Rate limits: 5 attempts per email per 5min, 10 attempts per IP per 5min.

## CSRF Protection

When `AUTH_MODE=cookie`, state-changing endpoints (POST/PUT/DELETE/PATCH) require an `X-CSRF-Token` header matching the `csrf_token` cookie. Double-submit pattern with constant-time comparison.

Bearer mode does not require CSRF — the browser cannot implicitly send the `Authorization` header.

## Python Sidecar HMAC Trust

The Rust backend forwards authenticated user identity to the Python sidecar over a Unix socket via signed headers:

```
X-User-Id: <user_id>
X-User-Email: <email>
X-User-Name: <name>
X-Auth-Signature: HMAC-SHA256(SIDECAR_SHARED_SECRET, "{user_id}|{email}|{name}")
```

The sidecar validates the signature on every non-public request. Set `SIDECAR_SHARED_SECRET` in `.env` and ensure both processes share the same value.

## Session Config

Sessions are stored in Redis with configurable TTL. Session cookies are `HttpOnly`, `SameSite=Lax`, `Path=/`.

Logout destroys the Redis session, blacklists the JWT's JTI for the remaining token lifetime, and clears the session cookie.
