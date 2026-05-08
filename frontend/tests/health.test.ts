import { describe, expect, test, mock } from "bun:test";
import { aggregateHealth, isFullOutage, getDiagnostics, createRetryController } from "../src/lib/health";

function makeResponse(json: () => Promise<unknown>): Response {
  return {
    json,
    ok: true,
    status: 200,
    headers: new Headers(),
    redirected: false,
    statusText: "OK",
    type: "basic",
    url: "",
  } as unknown as Response;
}

describe("aggregateHealth", () => {
  test("all endpoints healthy returns ok statuses", async () => {
    const fetchImpl = mock(async (url: string, _init?: RequestInit) => {
      // /health returns nested { rust: { status }, db, redis, ... }
      if ((url as string).endsWith("/health")) {
        return makeResponse(async () => ({
          rust: { status: "ok", service: "api", version: "0.1.0" },
          db: { status: "ok" },
          redis: { status: "ok" },
          storage: { status: "ok" },
          python: { status: "ok" },
          auth: { status: "ok" },
        }));
      }
      return makeResponse(async () => ({ status: "ok" }));
    });
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect((result.rust as Record<string, unknown>).status).toBe("ok");
    expect((result.db as Record<string, unknown>).status).toBe("ok");
    expect((result.python as Record<string, unknown>).status).toBe("ok");
  });

  test("rust fetch rejection returns error status", async () => {
    const fetchImpl = mock(async () => {
      throw new Error("connect ECONNREFUSED");
    });
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect((result.rust as Record<string, unknown>).status).toBe("error");
  });

  test("rust JSON parse failure returns error status", async () => {
    const fetchImpl = mock(async () =>
      makeResponse(async () => {
        throw new SyntaxError("Unexpected token");
      }),
    );
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect((result.rust as Record<string, unknown>).status).toBe("error");
  });

  test("python parse failure returns unavailable status", async () => {
    const fetchImpl = mock(async (url: string, _init?: RequestInit) => {
      if ((url as string).endsWith("/health/python")) {
        return makeResponse(async () => {
          throw new SyntaxError("bad JSON");
        });
      }
      // /health returns nested structure
      if ((url as string).endsWith("/health")) {
        return makeResponse(async () => ({
          rust: { status: "ok", service: "api", version: "0.1.0" },
          db: { status: "ok" },
          redis: { status: "ok" },
          storage: { status: "ok" },
          python: { status: "ok" },
          auth: { status: "ok" },
        }));
      }
      return makeResponse(async () => ({ status: "ok" }));
    });
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect((result.python as Record<string, unknown>).status).toBe("unavailable");
  });

  test("mixed: rust ok, db error, python unavailable", async () => {
    const fetchImpl = mock(async (url: string) => {
      // /health returns nested structure with rust ok
      if ((url as string).endsWith("/health")) {
        return makeResponse(async () => ({
          rust: { status: "ok", service: "api", version: "0.1.0" },
          db: { status: "ok" },
          redis: { status: "ok" },
          storage: { status: "ok" },
          python: { status: "ok" },
          auth: { status: "ok" },
        }));
      }
      if ((url as string).endsWith("/health/db")) throw new Error("db down");
      return makeResponse(async () => ({ status: "unavailable" }));
    });
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect((result.rust as Record<string, unknown>).status).toBe("ok");
    expect((result.db as Record<string, unknown>).status).toBe("error");
    expect((result.python as Record<string, unknown>).status).toBe("unavailable");
  });

  describe("diagnostics", () => {
    test("isFullOutage true when all 6 are error", () => {
      const data: Record<string, unknown> = {};
      for (const svc of ["rust", "db", "redis", "storage", "python", "auth"]) {
        data[svc] = { status: "error" };
      }
      expect(isFullOutage(data)).toBe(true);
    });

    test("isFullOutage false when some are ok", () => {
      const data: Record<string, unknown> = {
        rust: { status: "ok" },
        db: { status: "error" },
        redis: { status: "unavailable" },
        storage: { status: "error" },
        python: { status: "error" },
        auth: { status: "disabled" },
      };
      expect(isFullOutage(data)).toBe(false);
    });

    test("isFullOutage false when all ok", () => {
      const data: Record<string, unknown> = {};
      for (const svc of ["rust", "db", "redis", "storage", "python", "auth"]) {
        data[svc] = { status: "ok" };
      }
      expect(isFullOutage(data)).toBe(false);
    });

    test("getDiagnostics extracts fix fields", () => {
      const data: Record<string, unknown> = {
        rust: { status: "ok" },
        db: { status: "error", fix: "make up-postgres" },
        redis: { status: "ok" },
        storage: { status: "error", error: "S3 timeout" },
        python: { status: "unavailable", fix: "check sidecar logs" },
        auth: { status: "ok" },
      };
      const diag = getDiagnostics(data);
      expect(diag).toHaveLength(3);
      expect(diag[0].fix).toBe("make up-postgres");
      expect(diag[1].fix).toBe("S3 timeout");
      expect(diag[2].fix).toBe("check sidecar logs");
    });

    test("getDiagnostics uses error field when fix absent", () => {
      const data: Record<string, unknown> = {
        rust: { status: "error", error: "connection refused" },
        db: { status: "ok" },
        redis: { status: "ok" },
        storage: { status: "ok" },
        python: { status: "ok" },
        auth: { status: "ok" },
      };
      const diag = getDiagnostics(data);
      expect(diag).toHaveLength(1);
      expect(diag[0].fix).toBe("connection refused");
    });

    test("getDiagnostics returns null fix when neither field present", () => {
      const data: Record<string, unknown> = {
        rust: { status: "error" },
        db: { status: "ok" },
        redis: { status: "ok" },
        storage: { status: "ok" },
        python: { status: "ok" },
        auth: { status: "ok" },
      };
      const diag = getDiagnostics(data);
      expect(diag).toHaveLength(1);
      expect(diag[0].fix).toBeNull();
    });

    test("createRetryController starts timer and calls callback", async () => {
      const calls: number[] = [];
      const ctrl = createRetryController(() => { calls.push(Date.now()); }, 50000, 50);
      expect(calls.length).toBe(0);
      ctrl.start();
      await new Promise((r) => setTimeout(r, 80));
      expect(calls.length).toBe(1);
      ctrl.cancel();
    });

    test("createRetryController cancel stops further calls", async () => {
      const calls: number[] = [];
      const ctrl = createRetryController(() => { calls.push(Date.now()); }, 50000, 50);
      ctrl.start();
      await new Promise((r) => setTimeout(r, 20));
      ctrl.cancel();
      await new Promise((r) => setTimeout(r, 100));
      expect(calls.length).toBe(0);
    });

    test("createRetryController reset clears and restarts delay", async () => {
      const calls: number[] = [];
      const ctrl = createRetryController(() => { calls.push(Date.now()); }, 50000, 50);
      ctrl.start();
      await new Promise((r) => setTimeout(r, 80));
      expect(calls.length).toBe(1);
      ctrl.reset();
      await new Promise((r) => setTimeout(r, 40));
      // After reset, timer was cancelled — no more calls within 40ms
      expect(calls.length).toBe(1);
    });
  });

  test("x-trace-id header is sent in each fetch call", async () => {
    const capturedHeaders: Record<string, string>[] = [];
    const fetchImpl = mock(async (url: string, init?: RequestInit) => {
      capturedHeaders.push((init?.headers as Record<string, string>) || {});
      // /health returns nested structure
      if ((url as string).endsWith("/health")) {
        return makeResponse(async () => ({
          rust: { status: "ok", service: "api", version: "0.1.0" },
          db: { status: "ok" },
          redis: { status: "ok" },
          storage: { status: "ok" },
          python: { status: "ok" },
          auth: { status: "ok" },
        }));
      }
      return makeResponse(async () => ({ status: "ok" }));
    });

    await aggregateHealth(fetchImpl as unknown as typeof fetch);

    expect(capturedHeaders.length).toBe(6);
    for (const headers of capturedHeaders) {
      expect(headers["x-trace-id"]).toBeString();
      expect(headers["x-trace-id"]!.length).toBeGreaterThan(0);
    }
  });

  test("result contains entries for all services", async () => {
    const fetchImpl = mock(async (url: string) => {
      // /health returns nested structure
      if ((url as string).endsWith("/health")) {
        return makeResponse(async () => ({
          rust: { status: "ok", service: "api", version: "0.1.0" },
          db: { status: "ok" },
          redis: { status: "ok" },
          storage: { status: "ok" },
          python: { status: "ok" },
          auth: { status: "ok" },
        }));
      }
      return makeResponse(async () => ({ status: "ok" }));
    });
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect(result.rust).toBeDefined();
    expect(result.db).toBeDefined();
    expect(result.python).toBeDefined();
  });
});
