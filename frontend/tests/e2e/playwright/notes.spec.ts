import { test, expect } from "@playwright/test";

test.describe("Notes CRUD", () => {
  const title = `Test Note ${Date.now()}`;
  const body = "Playwright test body content";

  test("create note via form and see it in list", async ({ page }) => {
    // Navigate to notes — may redirect to login first
    await page.goto("/notes");

    // If login page appears, authenticate
    if (page.url().includes("/login")) {
      await page.fill('input[name="email"]', "test@example.com");
      await page.fill('input[name="password"]', "password123");
      await page.click('button[type="submit"]');
      await page.waitForURL("/notes", { timeout: 10000 });
    }

    // Wait for page to load (either empty state or table)
    await page.waitForSelector("#notes-loading", { state: "hidden", timeout: 10000 }).catch(() => {});
    await page.waitForTimeout(500);

    // Click "New Note" button
    await page.click('a[href="/notes/create"]');
    await page.waitForURL("/notes/create");

    // Fill form
    await page.fill('input[name="title"]', title);
    await page.fill('textarea[name="body"]', body);
    await page.click('button[type="submit"]');

    // Should redirect back to /notes
    await page.waitForURL("/notes", { timeout: 10000 });

    // Verify note title appears in list
    await expect(page.locator("text=" + title)).toBeVisible({ timeout: 10000 });
  });

  test("view note detail and delete", async ({ page }) => {
    await page.goto("/notes");

    // Authenticate if needed
    if (page.url().includes("/login")) {
      await page.fill('input[name="email"]', "test@example.com");
      await page.fill('input[name="password"]', "password123");
      await page.click('button[type="submit"]');
      await page.waitForURL("/notes", { timeout: 10000 });
    }

    // Wait for notes to load
    await page.waitForSelector("#notes-loading", { state: "hidden", timeout: 10000 }).catch(() => {});

    // Click the first note link
    const firstNote = page.locator('a[href^="/notes/"]').first();
    await expect(firstNote).toBeVisible({ timeout: 10000 });

    // Grab the note id from the href
    const href = await firstNote.getAttribute("href");
    await firstNote.click();
    await page.waitForURL(href!);

    // Click delete button
    await page.click("#delete-btn");

    // Modal should be visible — confirm delete
    await page.click("#confirm-delete");

    // Should redirect to /notes
    await page.waitForURL("/notes", { timeout: 10000 });

    // Toast should have appeared
    const toast = page.locator("toast-container .toast-item");
    await expect(toast).toBeVisible({ timeout: 5000 });

    // Toast should eventually disappear
    await expect(toast).not.toBeVisible({ timeout: 10000 });
  });
});
