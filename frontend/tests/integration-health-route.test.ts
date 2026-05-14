import { describe, expect, test, vi } from "vitest";

/**
 * Integration tests for the /api/health aggregation route.
 *
 * The handler (`src/pages/api/health.ts`) calls backend endpoints
 * and fans the results into a single JSON response. We mock `fetch` at the
 * module level so no real network is required.
 */

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type FetchResponses = {
  health?: object | Error;
  healthDb?: object | Error;
  healthPython?: object | Error;
};

function makeFetchMock(responses: FetchResponses) {
  return vi.fn(async (url: string) => {
    const u = url.toString();

    const pick = (val: object | Error | undefined) => {
      if (val instanceof Error) throw val;
      return new Response(JSON.stringify(val ?? {}), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    };

    if (u.endsWith("/health/db")) return pick(responses.healthDb);
    if (u.endsWith("/health/python")) return pick(responses.healthPython);
    if (u.endsWith("/health")) {
      const val = responses.health;
      if (val instanceof Error) throw val;
      const inner = val ?? {};
      return new Response(JSON.stringify({ rust: inner }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }

    return new Response(JSON.stringify({ status: "unknown" }), { status: 404 });
  });
}

import { aggregateHealth } from "../src/lib/health";
import { makeFetch } from "./test-utils";

// Test helper: wraps aggregateHealth into a Response matching the Astro route shape
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

describe("/api/health aggregation route", () => {
  describe("happy path — all backends healthy", () => {
    test("returns 200", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      expect(response.status).toBe(200);
    });

    test("content-type is application/json", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      expect(response.headers.get("content-type")).toContain(
        "application/json",
      );
    });

    test("body contains rust, db, and python keys", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<string, unknown>;

      expect(body).toHaveProperty("rust");
      expect(body).toHaveProperty("db");
      expect(body).toHaveProperty("python");
    });

    test("rust status is forwarded correctly", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.rust.status).toBe("ok");
    });

    test("db payload is forwarded as-is", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.db.status).toBe("ok");
    });

    test("python payload is forwarded as-is", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.python.status).toBe("ok");
    });
  });

  describe("degraded path — rust backend unreachable", () => {
    test("rust status becomes 'error' on fetch failure", async () => {
      const fetchMock = makeFetchMock({
        health: new Error("connection refused"),
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.rust.status).toBe("error");
    });

    test("db and python still reflect real values when only rust fails", async () => {
      const fetchMock = makeFetchMock({
        health: new Error("connection refused"),
        healthDb: { status: "ok" },
        healthPython: { status: "unavailable", error: "socket not found" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.db.status).toBe("ok");
      expect(body.python.status).toBe("unavailable");
    });
  });

  describe("degraded path — db unreachable", () => {
    test("db status becomes 'error' on fetch failure", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: new Error("connection refused"),
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.db.status).toBe("error");
    });
  });

  describe("degraded path — python sidecar unreachable", () => {
    test("python status becomes 'unavailable' on fetch failure", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: new Error("connection refused"),
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.python.status).toBe("unavailable");
    });
  });

  describe("total outage — all backends unreachable", () => {
    test("all statuses reflect failure modes", async () => {
      const fetchMock = makeFetchMock({
        health: new Error("ECONNREFUSED"),
        healthDb: new Error("ECONNREFUSED"),
        healthPython: new Error("ECONNREFUSED"),
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.rust.status).toBe("error");
      expect(body.db.status).toBe("error");
      expect(body.python.status).toBe("unavailable");
    });

    test("still returns 200 — health route itself does not error out", async () => {
      const fetchMock = makeFetchMock({
        health: new Error("ECONNREFUSED"),
        healthDb: new Error("ECONNREFUSED"),
        healthPython: new Error("ECONNREFUSED"),
      });

      const response = await runHandler(makeFetch(fetchMock));
      expect(response.status).toBe(200);
    });
  });

  describe("parallelism — all three fetches issued", () => {
    test("fetch called exactly six times with correct URLs", async () => {
      const fetchCalls: string[] = [];
      const fetchMock = vi.fn(async (url: string) => {
        fetchCalls.push(url);
        if (url.endsWith("/health")) {
          return new Response(
            JSON.stringify({
              rust: { status: "ok", service: "api", version: "0.1.0" },
              db: { status: "ok" },
              redis: { status: "ok" },
              storage: { status: "ok" },
              python: { status: "ok" },
              auth: { status: "ok" },
            }),
            {
              status: 200,
              headers: { "Content-Type": "application/json" },
            },
          );
        }
        return new Response(JSON.stringify({ status: "ok" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      });

      await runHandler(makeFetch(fetchMock));

      expect(fetchCalls).toHaveLength(6);
      expect(fetchCalls[0]).toMatch(/\/health$/);
      expect(fetchCalls[1]).toMatch(/\/health\/db$/);
      expect(fetchCalls[2]).toMatch(/\/health\/redis$/);
      expect(fetchCalls[3]).toMatch(/\/health\/storage$/);
      expect(fetchCalls[4]).toMatch(/\/health\/python$/);
    });
  });

  describe("malformed JSON response", () => {
    test("handles valid HTTP response with non-JSON body", async () => {
      const fetchMock = vi.fn(async () => {
        return new Response("not-json", { status: 200 });
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.rust.status).toBe("error");
      expect(body.db.status).toBe("error");
      expect(body.python.status).toBe("unavailable");
    });
  });

  describe("partial malformed JSON — one endpoint returns non-JSON", () => {
    test("rust returns non-JSON, db and python succeed", async () => {
      const fetchMock = vi.fn(async (url: string) => {
        if (url.endsWith("/health"))
          return new Response("not-json", { status: 200 });
        return new Response(JSON.stringify({ status: "ok" }), {
          headers: { "Content-Type": "application/json" },
        });
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.rust.status).toBe("error");
      expect(body.db.status).toBe("ok");
      expect(body.python.status).toBe("ok");
    });

    test("db returns non-JSON, rust and python succeed", async () => {
      const fetchMock = vi.fn(async (url: string) => {
        if (url.endsWith("/health/db"))
          return new Response("not-json", { status: 200 });
        if (url.endsWith("/health")) {
          return new Response(
            JSON.stringify({
              rust: { status: "ok", service: "api", version: "0.1.0" },
              db: { status: "ok" },
              redis: { status: "ok" },
              storage: { status: "ok" },
              python: { status: "ok" },
              auth: { status: "ok" },
            }),
            {
              headers: { "Content-Type": "application/json" },
            },
          );
        }
        return new Response(JSON.stringify({ status: "ok" }), {
          headers: { "Content-Type": "application/json" },
        });
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.rust.status).toBe("ok");
      expect(body.db.status).toBe("error");
      expect(body.python.status).toBe("ok");
    });

    test("python returns non-JSON, rust and db succeed", async () => {
      const fetchMock = vi.fn(async (url: string) => {
        if (url.endsWith("/health/python"))
          return new Response("not-json", { status: 200 });
        if (url.endsWith("/health")) {
          return new Response(
            JSON.stringify({
              rust: { status: "ok", service: "api", version: "0.1.0" },
              db: { status: "ok" },
              redis: { status: "ok" },
              storage: { status: "ok" },
              python: { status: "ok" },
              auth: { status: "ok" },
            }),
            {
              headers: { "Content-Type": "application/json" },
            },
          );
        }
        return new Response(JSON.stringify({ status: "ok" }), {
          headers: { "Content-Type": "application/json" },
        });
      });

      const response = await runHandler(makeFetch(fetchMock));
      const body = (await response.json()) as Record<
        string,
        Record<string, unknown>
      >;

      expect(body.rust.status).toBe("ok");
      expect(body.db.status).toBe("ok");
      expect(body.python.status).toBe("unavailable");
    });
  });
});
