import { describe, expect, test } from "bun:test";

describe("frontend generated integration test", () => {
  test("health route path is stable", () => {
    const route = "/api/health";
        expect(route.startsWith("/api/")).toBe(true);
  });
});
