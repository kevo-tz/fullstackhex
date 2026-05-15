import { test, expect } from "@playwright/test";

const SERVICE_IDS = ["rust", "db", "redis", "storage", "python", "auth"];

test.describe("Health Dashboard", () => {
  test("displays 6 service cards", async ({ page }) => {
    await page.goto("/");

    for (const id of SERVICE_IDS) {
      await expect(page.locator(`#card-${id}`)).toBeVisible();
      await expect(page.locator(`#dot-${id}`)).toBeVisible();
    }
  });

  test("status dots transition from loading to final state", async ({ page }) => {
    await page.goto("/");

    // Wait for health data to load — dots leave loading state
    for (const id of SERVICE_IDS) {
      const dot = page.locator(`#dot-${id}`);
      await expect(dot).not.toHaveClass(/loading/, { timeout: 15000 });
      // Dot should have one of: ok, error, degraded
      await expect(dot).toHaveClass(/dot (ok|error|degraded)/);
    }

    // Rust API and DB should be ok (core services running in CI)
    await expect(page.locator("#dot-rust")).toHaveClass(/dot ok/);
    await expect(page.locator("#dot-db")).toHaveClass(/dot ok/);
  });

  test("raw JSON block is populated after health fetch", async ({ page }) => {
    await page.goto("/");

    const rawJson = page.locator("#raw-json");
    // Initial state shows "fetching…"
    await expect(rawJson).toContainText("fetching");

    // Wait for health data to arrive
    await expect(rawJson).not.toContainText("fetching", { timeout: 15000 });
    // Should contain JSON content
    const text = await rawJson.textContent();
    expect(text).toBeTruthy();
    expect(text!.trim().startsWith("{")).toBe(true);
  });

  test("refresh button re-fetches health data", async ({ page }) => {
    await page.goto("/");

    // Wait for initial load
    await expect(page.locator("#dot-rust")).not.toHaveClass(/loading/, { timeout: 15000 });

    // Click refresh
    await page.click("#refresh-btn");

    // Dots briefly show loading again, then settle
    for (const id of ["rust", "db"]) {
      const dot = page.locator(`#dot-${id}`);
      await expect(dot).not.toHaveClass(/loading/, { timeout: 15000 });
      await expect(dot).toHaveClass(/dot ok/);
    }
  });
});
