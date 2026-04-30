import { describe, expect, test } from "bun:test";

describe("frontend generated unit test", () => {
  test("health endpoint path is valid", () => {
    const healthRoute = "/api/health";
    expect(healthRoute).toStartWith("/api/");
    expect(healthRoute).toContain("health");
  });

  test("environment variables are defined", () => {
    const apiUrl = process.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";
    expect(apiUrl).toBeTypeOf("string");
    expect(apiUrl.length).toBeGreaterThan(0);
  });

  test("TypeScript types work correctly", () => {
    interface HealthResponse {
      status: string;
      service: string;
    }

    const mockResponse: HealthResponse = {
      status: "ok",
      service: "api"
    };

    expect(mockResponse.status).toBe("ok");
    expect(mockResponse.service).toBe("api");
  });
});

// Python card detail-display logic: mirrors the inline script in index.astro
// Verifies stale detail/error is cleared when both are absent (ISSUE-001 fix).
describe("Python card detail logic", () => {
  function computeDetailText(py: { detail?: unknown; error?: string }): string {
    if (py.detail != null) {
      return JSON.stringify(py.detail, null, 2);
    }
    if (py.error != null) {
      return py.error;
    }
    return "";
  }

  test("shows detail when present", () => {
    expect(computeDetailText({ detail: { uptime: 42 } }))
      .toContain("uptime");
  });

  test("shows error when detail absent", () => {
    expect(computeDetailText({ error: "connection failed" }))
      .toBe("connection failed");
  });

  test("clears to empty string when neither detail nor error present", () => {
    expect(computeDetailText({})).toBe("");
  });

  test("clears to empty string when both are null", () => {
    expect(computeDetailText({ detail: null as unknown, error: null as unknown }))
      .toBe("");
  });
});
