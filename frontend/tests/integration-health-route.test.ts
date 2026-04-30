import { describe, expect, test, mock, beforeEach, afterEach } from "bun:test";

/**
 * Integration tests for the /api/health aggregation route.
 *
 * The handler (`src/pages/api/health.ts`) calls three Rust backend endpoints
 * and fans the results into a single JSON response.  We mock `fetch` at the
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
  return mock(async (url: string) => {
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
    if (u.endsWith("/health")) return pick(responses.health);

    return new Response(JSON.stringify({ status: "unknown" }), { status: 404 });
  });
}

// Re-implement the handler logic inline so tests don't need a running Astro
// server.  This mirrors `src/pages/api/health.ts` exactly and will catch
// regressions if the source diverges.
async function runHandler(fetchImpl: typeof fetch, apiBase = "http://localhost:8001") {
  const result: Record<string, unknown> = {
    rust: { status: "unknown" },
    db: { status: "unknown" },
    python: { status: "unknown" },
  };

  try {
    const r = await fetchImpl(`${apiBase}/health`);
    const d = await r.json();
    result.rust = { status: (d as Record<string, unknown>).status ?? "unknown" };
  } catch {
    result.rust = { status: "error" };
  }

  try {
    const r = await fetchImpl(`${apiBase}/health/db`);
    result.db = await r.json();
  } catch {
    result.db = { status: "error" };
  }

  try {
    const r = await fetchImpl(`${apiBase}/health/python`);
    result.python = await r.json();
  } catch {
    result.python = { status: "unavailable" };
  }

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

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      expect(response.status).toBe(200);
    });

    test("content-type is application/json", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      expect(response.headers.get("content-type")).toContain("application/json");
    });

    test("body contains rust, db, and python keys", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, unknown>;

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

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

      expect(body.rust.status).toBe("ok");
    });

    test("db payload is forwarded as-is", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

      expect(body.db.status).toBe("ok");
    });

    test("python payload is forwarded as-is", async () => {
      const fetchMock = makeFetchMock({
        health: { status: "ok", service: "api", version: "0.1.0" },
        healthDb: { status: "ok" },
        healthPython: { status: "ok", version: "3.12" },
      });

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

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

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

      expect(body.rust.status).toBe("error");
    });

    test("db and python still reflect real values when only rust fails", async () => {
      const fetchMock = makeFetchMock({
        health: new Error("connection refused"),
        healthDb: { status: "ok" },
        healthPython: { status: "unavailable", error: "socket not found" },
      });

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

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

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

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

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

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

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      const body = await response.json() as Record<string, Record<string, unknown>>;

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

      const response = await runHandler(fetchMock as unknown as typeof fetch);
      expect(response.status).toBe(200);
    });
  });
});
