import { describe, expect, test, vi } from "vitest";

/**
 * Integration tests for the /api/health aggregation route.
 *
 * aggregateHealth() makes a single GET /health request and extracts per-service
 * statuses from the response. We mock `fetch` so no real network is required.
 */

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type FetchOutcome = object | Error;

function makeFetchMock(healthOutcome: FetchOutcome) {
  return vi.fn(async (_url: string) => {
    if (healthOutcome instanceof Error) throw healthOutcome;
    return new Response(JSON.stringify(healthOutcome), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  });
}

import { aggregateHealth } from "../src/lib/health";
import { makeFetch } from "./test-utils";

async function runHandler(
  fetchImpl: typeof fetch,
  apiBase = "http://localhost:8001",
) {
  const result = await aggregateHealth(fetchImpl, apiBase);
  return new Response(JSON.stringify(result), {
    headers: { "Content-Type": "application/json" },
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

const ALL_OK = {
  rust: { status: "ok", service: "api" },
  db: { status: "ok" },
  redis: { status: "ok" },
  storage: { status: "ok" },
  python: { status: "ok" },
  auth: { status: "ok" },
};

describe("/api/health aggregation route", () => {
  describe("happy path — all backends healthy", () => {
    test("returns 200", async () => {
      const fetchMock = makeFetchMock(ALL_OK);
      const response = await runHandler(makeFetch(fetchMock));
      expect(response.status).toBe(200);
    });

    test("content-type is application/json", async () => {
      const fetchMock = makeFetchMock(ALL_OK);
      const response = await runHandler(makeFetch(fetchMock));
      expect(response.headers.get("content-type")).toContain("application/json");
    });

    test("body contains all service keys", async () => {
      const fetchMock = makeFetchMock(ALL_OK);
      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<string, unknown>;
      for (const svc of ["rust", "db", "redis", "storage", "python", "auth"]) {
        expect(body).toHaveProperty(svc);
      }
    });

    test("service statuses forwarded correctly", async () => {
      const fetchMock = makeFetchMock(ALL_OK);
      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<string, Record<string, unknown>>;
      expect(body.rust.status).toBe("ok");
      expect(body.db.status).toBe("ok");
      expect(body.python.status).toBe("ok");
    });
  });

  describe("degraded path — per-service status from health response", () => {
    test("mixed statuses from single health response", async () => {
      const fetchMock = makeFetchMock({
        rust: { status: "ok", service: "api" },
        db: { status: "error" },
        redis: { status: "ok" },
        storage: { status: "error" },
        python: { status: "unavailable" },
        auth: { status: "disabled" },
      });
      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<string, Record<string, unknown>>;
      expect(body.rust.status).toBe("ok");
      expect(body.db.status).toBe("error");
      expect(body.python.status).toBe("unavailable");
      expect(body.auth.status).toBe("disabled");
    });
  });

  describe("total outage — fetch fails", () => {
    test("all statuses reflect failure on fetch error", async () => {
      const fetchMock = makeFetchMock(new Error("ECONNREFUSED"));
      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<string, Record<string, unknown>>;
      expect(body.rust.status).toBe("error");
      expect(body.db.status).toBe("error");
      expect(body.python.status).toBe("unavailable");
    });

    test("still returns 200 — health route itself does not error out", async () => {
      const fetchMock = makeFetchMock(new Error("ECONNREFUSED"));
      const response = await runHandler(makeFetch(fetchMock));
      expect(response.status).toBe(200);
    });
  });

  describe("single fetch call", () => {
    test("fetch called exactly once with correct URL", async () => {
      const fetchCalls: string[] = [];
      const fetchMock = vi.fn(async (url: string) => {
        fetchCalls.push(url);
        return new Response(JSON.stringify(ALL_OK), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      });

      await runHandler(makeFetch(fetchMock));
      expect(fetchCalls).toHaveLength(1);
      expect(fetchCalls[0]).toMatch(/\/health$/);
    });
  });

  describe("malformed JSON response", () => {
    test("handles valid HTTP response with non-JSON body", async () => {
      const fetchMock = vi.fn(async () => {
        return new Response("not-json", { status: 200 });
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<string, Record<string, unknown>>;
      expect(body.rust.status).toBe("error");
      expect(body.db.status).toBe("error");
      expect(body.python.status).toBe("unavailable");
    });
  });
});
