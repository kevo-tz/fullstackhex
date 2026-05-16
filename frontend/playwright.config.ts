import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e/playwright",
  timeout: 30000,
  workers: 1,
  globalSetup: "./tests/e2e/playwright/global-setup.ts",
  use: {
    baseURL: process.env.FRONTEND_URL || "http://localhost:4321",
    headless: true,
  },
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium" },
    },
  ],
});
