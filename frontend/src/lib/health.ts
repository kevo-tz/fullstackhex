function jsonLog(obj: Record<string, unknown>): void {
  console.log(JSON.stringify(obj));
}

export async function aggregateHealth(
  fetchImpl: typeof fetch,
  apiBase = "http://localhost:8001",
): Promise<Record<string, unknown>> {
  const traceId = crypto.randomUUID();
  const start = performance.now();

  jsonLog({
    timestamp: new Date().toISOString(),
    level: "info",
    target: "frontend:health",
    message: "health check fan-out",
    trace_id: traceId,
  });

  const result: Record<string, unknown> = {
    rust: { status: "unknown" },
    db: { status: "unknown" },
    python: { status: "unknown" },
  };

  const [rustRes, dbRes, pythonRes] = await Promise.allSettled([
    fetchImpl(`${apiBase}/health`),
    fetchImpl(`${apiBase}/health/db`),
    fetchImpl(`${apiBase}/health/python`),
  ]);

  if (rustRes.status === "fulfilled") {
    try {
      const d = await rustRes.value.json();
      result.rust = { status: (d as Record<string, unknown>).status ?? "unknown" };
      jsonLog({
        timestamp: new Date().toISOString(),
        level: "info",
        target: "frontend:health",
        message: "rust health response",
        trace_id: traceId,
        target_service: "api",
        response_status: (d as Record<string, unknown>).status,
      });
    } catch {
      result.rust = { status: "error" };
    }
  } else {
    result.rust = { status: "error" };
  }

  if (dbRes.status === "fulfilled") {
    try {
      result.db = await dbRes.value.json();
      jsonLog({
        timestamp: new Date().toISOString(),
        level: "info",
        target: "frontend:health",
        message: "db health response",
        trace_id: traceId,
        target_service: "db",
        response_status: (result.db as Record<string, unknown>).status,
      });
    } catch {
      result.db = { status: "error" };
    }
  } else {
    result.db = { status: "error" };
  }

  if (pythonRes.status === "fulfilled") {
    try {
      result.python = await pythonRes.value.json();
      jsonLog({
        timestamp: new Date().toISOString(),
        level: "info",
        target: "frontend:health",
        message: "python health response",
        trace_id: traceId,
        target_service: "python",
        response_status: (result.python as Record<string, unknown>).status,
      });
    } catch {
      result.python = { status: "unavailable" };
    }
  } else {
    result.python = { status: "unavailable" };
  }

  const durationMs = Math.round(performance.now() - start);
  jsonLog({
    timestamp: new Date().toISOString(),
    level: "info",
    target: "frontend:health",
    message: "health check complete",
    trace_id: traceId,
    duration_ms: durationMs,
  });

  return result;
}
