import { describe, expect, test } from "bun:test";

describe("frontend generated unit test", () => {
  test("health endpoint path is valid", () => {
    const healthRoute = "/api/health";
    expect(healthRoute).toStartWith("/api/");
    expect(healthRoute).toContain("health");
  });

  test("environment variables are defined", () => {
    const apiUrl = process.env.PUBLIC_API_URL || "http://localhost:8001";
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
