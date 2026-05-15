import { request } from "@playwright/test";

export interface TestUser {
  email: string;
  password: string;
  name: string;
  accessToken: string;
  userId: string;
}

/**
 * Register a unique test user via the API proxy.
 * Used in beforeAll to set up credentials for form-based login tests.
 */
export async function registerTestUser(): Promise<TestUser> {
  const baseURL = process.env.FRONTEND_URL || "http://localhost:4321";
  const email = `e2e-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@test.example.com`;
  const password = "e2e-test-password-123";
  const name = "E2E Test User";

  const ctx = await request.newContext({ baseURL });
  const res = await ctx.post("/api/auth/register", {
    data: { email, password, name },
  });

  if (res.status() === 429) {
    throw new Error(`Rate-limited during test user registration: ${await res.text()}`);
  }
  if (res.status() !== 201) {
    throw new Error(`Registration failed (${res.status()}): ${await res.text()}`);
  }

  const data = await res.json();
  return {
    email,
    password,
    name,
    accessToken: data.access_token,
    userId: data.user.id,
  };
}
