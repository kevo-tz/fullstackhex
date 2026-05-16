import { request } from "@playwright/test";
import { writeFileSync } from "fs";
import type { TestUser } from "./auth-helpers";

async function globalSetup() {
  const baseURL = process.env.FRONTEND_URL || "http://localhost:4321";
  const email = `e2e-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@test.example.com`;
  const password = "e2e-test-password-123";
  const name = "E2E Test User";

  const ctx = await request.newContext({ baseURL });
  const res = await ctx.post("/api/auth/register", {
    data: { email, password, name },
  });

  if (res.status() !== 201) {
    const body = await res.text();
    throw new Error(`Global setup registration failed (${res.status()}): ${body}`);
  }

  const data = await res.json();
  const testUser: TestUser = {
    email,
    password,
    name,
    accessToken: data.access_token,
    userId: data.user.id,
  };

  writeFileSync("e2e-test-user.json", JSON.stringify(testUser), "utf-8");
}

export default globalSetup;
