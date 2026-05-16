import { test, expect } from "@playwright/test";
import { getSharedTestUser, registerTestUser } from "./auth-helpers";

test.describe("Notes CRUD", () => {
  const title = `Test Note ${Date.now()}`;
  const body = "Playwright test body content";
  let testUser: Awaited<ReturnType<typeof registerTestUser>>;

  test.beforeAll(async () => {
    testUser = getSharedTestUser() ?? await registerTestUser();
  });

  async function authenticate(page: import("@playwright/test").Page) {
    await page.goto("/login");
    await page.fill('input[name="email"]', testUser.email);
    await page.fill('input[name="password"]', testUser.password);
    await page.click('button[type="submit"]');
    await page.waitForURL("/profile", { timeout: 15000 });
  }

  test("create note via form and see it in list", async ({ page }) => {
    await authenticate(page);
    await page.goto("/notes");

    await page.waitForSelector("#notes-loading", { state: "hidden", timeout: 10000 }).catch(() => {});
    await page.waitForTimeout(500);

    await page.click('a[href="/notes/create"]');
    await page.waitForURL("/notes/create");

    await page.fill('input[name="title"]', title);
    await page.fill('textarea[name="body"]', body);
    await page.click('button[type="submit"]');

    await page.waitForURL("/notes", { timeout: 10000 });
    await expect(page.locator("text=" + title)).toBeVisible({ timeout: 10000 });
  });

  test("view note detail and delete", async ({ page }) => {
    page.on("response", (res) => {
      console.log("API:", res.status(), res.url());
    });
    page.on("pageerror", (err) => {
      console.log("PAGE ERROR:", err.message);
    });

    await authenticate(page);
    await page.goto("/notes");

    await page.waitForSelector("#notes-loading", { state: "hidden", timeout: 10000 }).catch(() => {});

    const firstNote = page.locator('a[href^="/notes/"]').first();
    await expect(firstNote).toBeVisible({ timeout: 10000 });

    const href = await firstNote.getAttribute("href");
    console.log("Navigating to:", href);
    await firstNote.click();
    await page.waitForURL(href!);
    console.log("After nav URL:", page.url());

    // Wait for content or error
    try {
      await page.waitForSelector("#detail-content:not(.hidden)", { timeout: 10000 });
    } catch {
      const errVisible = await page.locator("#detail-error:not(.hidden)").count();
      console.log("detail-error visible:", errVisible);
      const errMsg = await page.locator("#detail-error-msg").textContent();
      console.log("detail-error msg:", errMsg);
      throw new Error("Note detail did not load");
    }

    await page.click("#delete-btn");
    await page.click("#confirm-delete");

    await page.waitForURL("/notes", { timeout: 10000 });

    const toast = page.locator("toast-container .toast-item");
    await expect(toast).toBeVisible({ timeout: 5000 });
    await expect(toast).not.toBeVisible({ timeout: 10000 });
  });
});
