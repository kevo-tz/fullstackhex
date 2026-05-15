import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e/playwright",
  timeout: 30000,
  use: {
    baseURL: process.env.FRONTEND_URL || "http://localhost:4321",
    headless: true,
  },
  projects: [
    {
      name: "chromium",
      use: { browserName: "chromium" },
    },
    {
      name: "firefox",
      use: { browserName: "firefox" },
    },
    {
      name: "webkit",
      use: { browserName: "webkit" },
    },
  ],
});
