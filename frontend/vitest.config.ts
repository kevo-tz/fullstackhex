import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["tests/**/*.test.ts"],
    exclude: ["**/tests/e2e/**", "**/node_modules/**", "**/dist/**"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts", "src/**/*.astro"],
      reporter: ["text", "json"],
    },
  },
});
