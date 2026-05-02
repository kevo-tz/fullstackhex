import { describe, expect, test, mock } from "bun:test";
import { aggregateHealth } from "../src/lib/health";

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
    const fetchImpl = mock(async (_url: string, _init?: RequestInit) =>
      makeResponse(async () => ({ status: "ok" })),
    );
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
      return makeResponse(async () => ({ status: "ok" }));
    });
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect((result.python as Record<string, unknown>).status).toBe("unavailable");
  });

  test("mixed: rust ok, db error, python unavailable", async () => {
    let calls = 0;
    const fetchImpl = mock(async () => {
      calls++;
      if (calls === 1) return makeResponse(async () => ({ status: "ok" }));
      if (calls === 2) throw new Error("db down");
      return makeResponse(async () => ({ status: "unavailable" }));
    });
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect((result.rust as Record<string, unknown>).status).toBe("ok");
    expect((result.db as Record<string, unknown>).status).toBe("error");
    expect((result.python as Record<string, unknown>).status).toBe("unavailable");
  });

  test("x-trace-id header is sent in each fetch call", async () => {
    const capturedHeaders: Record<string, string>[] = [];
    const fetchImpl = mock(async (_url: string, init?: RequestInit) => {
      capturedHeaders.push((init?.headers as Record<string, string>) || {});
      return makeResponse(async () => ({ status: "ok" }));
    });

    await aggregateHealth(fetchImpl as unknown as typeof fetch);

    expect(capturedHeaders.length).toBe(3);
    for (const headers of capturedHeaders) {
      expect(headers["x-trace-id"]).toBeString();
      expect(headers["x-trace-id"]!.length).toBeGreaterThan(0);
    }
  });

  test("result contains entries for all services", async () => {
    const fetchImpl = mock(async () =>
      makeResponse(async () => ({ status: "ok" })),
    );
    const result = await aggregateHealth(fetchImpl as unknown as typeof fetch);
    expect(result.rust).toBeDefined();
    expect(result.db).toBeDefined();
    expect(result.python).toBeDefined();
  });
});
