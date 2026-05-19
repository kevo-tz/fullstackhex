import { request } from "@playwright/test";
import { readFileSync, unlinkSync } from "fs";
import type { TestUser } from "./auth-helpers";

async function globalTeardown() {
  const baseURL = process.env.FRONTEND_URL || "http://localhost:4321";
  let testUser: TestUser | null = null;

  try {
    const data = readFileSync("e2e-test-user.json", "utf-8");
    testUser = JSON.parse(data) as TestUser;
  } catch {
    // No shared user to clean up
    return;
  }

  try {
    const ctx = await request.newContext({ baseURL });
    const res = await ctx.delete("/api/auth/me", {
      headers: {
        Authorization: `Bearer ${testUser.accessToken}`,
      },
    });
    if (!res.ok && res.status() !== 401) {
      console.warn(`Teardown: DELETE /auth/me returned ${res.status()}`);
    }
  } catch (err) {
    console.warn("Teardown error:", err instanceof Error ? err.message : String(err));
  }

  try {
    unlinkSync("e2e-test-user.json");
  } catch {
    // Best-effort cleanup of temp file
  }
}

export default globalTeardown;
