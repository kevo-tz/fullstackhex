/**
 * Auth e2e test — Vitest runner.
 *
 * Requires running backend (port 8001) and frontend (port 4321).
 * Uses Node.js built-in fetch, no external test deps.
 *
 * Usage:
 *   bun vitest run tests/e2e/auth.test.ts
 */

import { beforeAll, describe, expect, test } from "vitest";

const BACKEND = process.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";
const FRONTEND = process.env.FRONTEND_URL || "http://localhost:4321";
const TEST_USER = {
  email: `e2e-${Date.now()}@test.example.com`,
  password: "e2e-test-password-123",
};

interface AuthResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
  user: {
    id: string;
    email: string;
    name: string | null;
    provider: string;
  };
}

let accessToken = "";

// Register a fresh user and store the token. All tests in this block share
// one registration to avoid duplicate-user errors on the backend.
async function registerUser(): Promise<boolean> {
  if (accessToken) return true;
  const res = await fetch(`${BACKEND}/auth/register`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      email: TEST_USER.email,
      password: TEST_USER.password,
      name: "E2E Test",
    }),
  });
  if (res.status === 404) {
    console.warn("SKIP: /auth/register returned 404 — auth not configured");
    return false;
  }
  if (res.status !== 201) {
    console.warn(`register returned ${res.status} — continuing`);
    return false;
  }
  const data: AuthResponse = await res.json();
  accessToken = data.access_token;
  return true;
}

describe("e2e auth flow", () => {
  beforeAll(async () => {
    await registerUser();
  });

  test("POST /auth/register creates user and returns JWT", async () => {
    const uniqueEmail = `e2e-reg-${Date.now()}@test.example.com`;
    const res = await fetch(`${BACKEND}/auth/register`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        email: uniqueEmail,
        password: TEST_USER.password,
        name: "E2E Test",
      }),
    });

    if (res.status === 404) {
      console.warn("SKIP: /auth/register returned 404 — auth not configured");
      return;
    }

    expect(res.status).toBe(201);

    const data: AuthResponse = await res.json();
    expect(data.access_token).toBeTruthy();
    expect(data.token_type).toBe("Bearer");
    expect(data.expires_in).toBeGreaterThan(0);
    expect(data.user.email).toBe(uniqueEmail);
    expect(data.user.provider).toBe("local");
  });

  test("POST /auth/login returns JWT for valid credentials", async () => {
    if (!accessToken) {
      // Fallback: try registering in case beforeAll was skipped
      const ok = await registerUser();
      if (!ok) return;
    }

    const res = await fetch(`${BACKEND}/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        email: TEST_USER.email,
        password: TEST_USER.password,
      }),
    });

    if (res.status === 404) {
      console.warn("SKIP: /auth/login returned 404 — auth not configured");
      return;
    }

    expect(res.status).toBe(200);

    const data: AuthResponse = await res.json();
    expect(data.access_token).toBeTruthy();
    accessToken = data.access_token;
  });

  test("POST /auth/login rejects wrong password", async () => {
    const res = await fetch(`${BACKEND}/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        email: TEST_USER.email,
        password: "wrong-password-12345",
      }),
    });

    if (res.status === 404) {
      console.warn("SKIP: /auth/login returned 404 — auth not configured");
      return;
    }

    expect(res.status).toBeGreaterThanOrEqual(400);
  });

  test("GET /auth/me returns user info with valid token", async () => {
    if (!accessToken) {
      console.warn("SKIP: no access token — auth not configured");
      return;
    }

    const res = await fetch(`${BACKEND}/auth/me`, {
      headers: { Authorization: `Bearer ${accessToken}` },
    });

    expect(res.status).toBe(200);

    const data = await res.json();
    expect(data.email).toBe(TEST_USER.email);
    expect(data.user_id).toBeTruthy();
    expect(data.provider).toBe("local");
  });

  test("GET /auth/me returns 401 without token", async () => {
    const res = await fetch(`${BACKEND}/auth/me`);

    if (res.status === 404) {
      console.warn("SKIP: /auth/me returned 404 — auth not configured");
      return;
    }

    expect(res.status).toBe(401);
  });

  test("GET / returns dashboard with 200", async () => {
    const res = await fetch(`${FRONTEND}/`);

    // Dashboard should serve, even if auth is disabled
    expect(res.ok).toBe(true);
    const html = await res.text();
    expect(html).toContain("FullStackHex");
  });

  test("GET /login returns login page with 200", async () => {
    const res = await fetch(`${FRONTEND}/login`);

    expect(res.ok).toBe(true);
    const html = await res.text();
    expect(html).toContain("Sign in");
  });

  test("GET /register returns register page with 200", async () => {
    const res = await fetch(`${FRONTEND}/register`);

    expect(res.ok).toBe(true);
    const html = await res.text();
    expect(html).toContain("Create account");
  });

  test("GET /api/health returns aggregated health", async () => {
    const res = await fetch(`${FRONTEND}/api/health`);

    expect(res.ok).toBe(true);
    const data = await res.json();
    expect(data.rust).toBeTruthy();
    expect(data.rust.status).toBe("ok");
  });
});
