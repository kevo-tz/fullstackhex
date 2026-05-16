import { test, expect } from "@playwright/test";
import { getSharedTestUser, registerTestUser } from "./auth-helpers";

test.describe("Auth Login", () => {
  let testUser: Awaited<ReturnType<typeof registerTestUser>>;

  test.beforeAll(async () => {
    testUser = getSharedTestUser() ?? await registerTestUser();
  });

  test("login form submits, redirects, and shows user info", async ({ page }) => {
      const logs: string[] = [];
      page.on("console", (msg) => {
        logs.push(`[${msg.type()}] ${msg.text()}`);
      });
      page.on("pageerror", (err) => {
        logs.push(`[pageerror] ${err.message}`);
      });

      await page.goto("/login");
      await page.waitForTimeout(500);

      console.log("Page title:", await page.title());
      console.log("Page URL:", page.url());

      // Fill form
      await page.fill('input[name="email"]', testUser.email);
      await page.fill('input[name="password"]', testUser.password);
      await page.click('button[type="submit"]');

      // Wait a bit then capture state
      await page.waitForTimeout(3000);
      console.log("URL after submit:", page.url());
      const errVisible = await page.locator("#auth-error.visible").count();
      console.log("auth-error visible:", errVisible);
      const errText = await page.locator("#auth-error").textContent();
      console.log("auth-error text:", errText);

      // Dump console logs
      for (const l of logs) console.log(l);

      // Should redirect to profile
      await page.waitForURL("/profile", { timeout: 15000 });

      // Profile content should be visible (not auth guard)
      await expect(page.locator("#profile-content")).toBeVisible();

      // User info should match registered user
      await expect(page.locator("#user-email")).toHaveText(testUser.email);
      await expect(page.locator("#user-name")).toHaveText(testUser.name);
      await expect(page.locator("#user-provider")).toHaveText("local");
    });

  test.describe("invalid credentials", () => {
    test("shows error and stays on login page", async ({ browser }) => {
      // Fresh context — no session cookies
      const context = await browser.newContext();
      const page = await context.newPage();

      await page.goto("/login");
      await page.fill('input[name="email"]', "nonexistent@test.com");
      await page.fill('input[name="password"]', "wrong-password");
      await page.click('button[type="submit"]');

      // Error message should appear
      await expect(page.locator("#auth-error")).toHaveClass(/visible/);
      // Should still be on login page
      expect(page.url()).toContain("/login");

      await context.close();
    });
  });
});
