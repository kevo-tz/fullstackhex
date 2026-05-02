function jsonLog(obj: Record<string, unknown>): void {
  console.log(JSON.stringify(obj));
}

async function handleService(
  fetchImpl: typeof fetch,
  url: string,
  serviceKey: string,
  targetService: string,
  defaultStatus: string,
  traceId: string,
): Promise<{ status: string }> {
  try {
    const res = await fetchImpl(url, {
      headers: { "x-trace-id": traceId },
    });
    const d = await res.json();
    const status = (d as Record<string, unknown>).status ?? "unknown";
    jsonLog({
      timestamp: new Date().toISOString(),
      level: "info",
      target: "frontend:health",
      message: `${serviceKey} health response`,
      trace_id: traceId,
      target_service: targetService,
      response_status: status,
    });
    return { status: String(status) };
  } catch {
    jsonLog({
      timestamp: new Date().toISOString(),
      level: "warn",
      target: "frontend:health",
      message: `${serviceKey} health fetch/parse failed`,
      trace_id: traceId,
    });
    return { status: defaultStatus };
  }
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

  const [rustResult, dbResult, pythonResult] = await Promise.all([
    handleService(fetchImpl, `${apiBase}/health`, "rust", "api", "error", traceId),
    handleService(fetchImpl, `${apiBase}/health/db`, "db", "db", "error", traceId),
    handleService(fetchImpl, `${apiBase}/health/python`, "python", "python", "unavailable", traceId),
  ]);

  const durationMs = Math.round(performance.now() - start);
  jsonLog({
    timestamp: new Date().toISOString(),
    level: "info",
    target: "frontend:health",
    message: "health check complete",
    trace_id: traceId,
    duration_ms: durationMs,
  });

  return {
    rust: rustResult,
    db: dbResult,
    python: pythonResult,
  };
}
