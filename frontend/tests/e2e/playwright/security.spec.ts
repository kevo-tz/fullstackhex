import { test, expect } from "@playwright/test";

const createdUsers: { email: string; password: string }[] = [];

test.afterAll(async ({ request }) => {
  for (const user of createdUsers) {
    try {
      const loginRes = await request.post("/api/auth/login", {
        data: { email: user.email, password: user.password },
      });
      if (!loginRes.ok) continue;
      const data = await loginRes.json();
      await request.delete("/api/auth/me", {
        headers: { Authorization: `Bearer ${data.access_token}` },
      });
    } catch {
      // Best-effort cleanup
    }
  }
});

test.describe("Security Headers", () => {
  test("CSP header has no unsafe-inline in script-src", async ({ page }) => {
    const response = await page.goto("/");
    if (!response) throw new Error("No response");
    const csp = response.headers()["content-security-policy"] || "";
    const scriptSrc = csp.split(";").find((s) => s.trim().startsWith("script-src"));
    if (scriptSrc) {
      expect(scriptSrc).not.toContain("unsafe-inline");
    }
  });
});

test.describe("Auth Cookie Security", () => {
  test("Set-Cookie headers include Secure flag on login", async ({ request }) => {
    const email = `sec-e2e-${Date.now()}@test.example.com`;
    const password = "e2e-test-password-123";
    createdUsers.push({ email, password });
    await request.post("/api/auth/register", {
      data: { email, password, name: "Security Test" },
    });
    const response = await request.post("/api/auth/login", {
      data: { email, password },
    });
    const setCookie = response.headers()["set-cookie"] || "";
    // In CI with production config, cookies should have Secure flag
    if (process.env.COOKIE_SECURE !== "false") {
      expect(setCookie).toContain("Secure");
    }
  });
});

test.describe("XSS Prevention", () => {
  test("note created_at with invalid date renders safe text", async ({ page, request }) => {
    const email = `xss-e2e-${Date.now()}@test.example.com`;
    const password = "e2e-test-password-123";
    createdUsers.push({ email, password });
    await request.post("/api/auth/register", {
      data: { email, password, name: "XSS Test" },
    });
    const loginRes = await request.post("/api/auth/login", {
      data: { email, password },
    });
    const loginData = await loginRes.json();
    const token = loginData.access_token;
    await request.post("/api/notes", {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: "Safe", body: "Test note", created_at: "<img src=x onerror=alert(1)>" },
    });
    await page.goto("/notes");
    await expect(page.locator("text=Safe")).toBeVisible();
    await expect(page.locator("text=<img")).toHaveCount(0);
  });
});
