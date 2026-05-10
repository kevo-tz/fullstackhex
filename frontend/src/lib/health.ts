export const SERVICE_IDS = ["rust", "db", "redis", "storage", "python", "auth"] as const;

function jsonLog(obj: Record<string, unknown>): void {
  if (typeof window !== "undefined" || import.meta.env.DEV) {
    console.log(JSON.stringify(obj));
  }
}

export function isFullOutage(data: Record<string, unknown>): boolean {
  for (const svc of SERVICE_IDS) {
  for (const svc of services) {
    const entry = data[svc] as Record<string, unknown> | undefined;
    if (!entry || entry.status === "ok") return false;
  }
  return true;
}

export function getDiagnostics(
  data: Record<string, unknown>,
): { service: string; status: string; fix: string | null }[] {
  const labels: Record<string, string> = {
    rust: "Rust API",
    db: "PostgreSQL",
    redis: "Redis",
    storage: "RustFS Storage",
    python: "Python sidecar",
    auth: "Auth",
  };
  const result: { service: string; status: string; fix: string | null }[] = [];
  for (const svc of SERVICE_IDS) {
    const entry = data[svc] as Record<string, unknown> | undefined;
    if (!entry || entry.status === "ok") continue;
    result.push({
      service: labels[svc] || svc,
      status: String(entry.status),
      fix: (entry.fix as string) || (entry.error as string) || null,
    });
  }
  return result;
}

export function createRetryController(
  onRetry: () => void,
  maxDelay = 30000,
  initialDelay = 1000,
): { start: () => void; cancel: () => void; reset: () => void } {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let delay = initialDelay;

  function schedule() {
    timer = setTimeout(() => {
      onRetry();
      if (delay < maxDelay) delay = Math.min(delay * 2, maxDelay);
      schedule();
    }, delay);
  }

  function doCancel() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  function doReset() {
    doCancel();
    delay = initialDelay;
  }

  function doStart() {
    doCancel();
    schedule();
  }

  return { start: doStart, cancel: doCancel, reset: doReset };
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

async function handleRustHealth(
  fetchImpl: typeof fetch,
  apiBase: string,
  traceId: string,
): Promise<{ status: string }> {
  try {
    const res = await fetchImpl(`${apiBase}/health`, {
      headers: { "x-trace-id": traceId },
    });
    const d = await res.json();
    const rust = (d as Record<string, unknown>).rust as
      | Record<string, unknown>
      | undefined;
    const status = rust?.status ?? "unknown";
    jsonLog({
      timestamp: new Date().toISOString(),
      level: "info",
      target: "frontend:health",
      message: "rust health response",
      trace_id: traceId,
      target_service: "api",
      response_status: status,
    });
    return { status: String(status) };
  } catch {
    jsonLog({
      timestamp: new Date().toISOString(),
      level: "warn",
      target: "frontend:health",
      message: "rust health fetch/parse failed",
      trace_id: traceId,
    });
    return { status: "error" };
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

  const [
    rustResult,
    dbResult,
    redisResult,
    storageResult,
    pythonResult,
    authResult,
  ] = await Promise.all([
    handleRustHealth(fetchImpl, apiBase, traceId),
    handleService(
      fetchImpl,
      `${apiBase}/health/db`,
      "db",
      "db",
      "error",
      traceId,
    ),
    handleService(
      fetchImpl,
      `${apiBase}/health/redis`,
      "redis",
      "redis",
      "unavailable",
      traceId,
    ),
    handleService(
      fetchImpl,
      `${apiBase}/health/storage`,
      "storage",
      "storage",
      "unavailable",
      traceId,
    ),
    handleService(
      fetchImpl,
      `${apiBase}/health/python`,
      "python",
      "python",
      "unavailable",
      traceId,
    ),
    handleService(
      fetchImpl,
      `${apiBase}/health/auth`,
      "auth",
      "auth",
      "disabled",
      traceId,
    ),
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
    redis: redisResult,
    storage: storageResult,
    python: pythonResult,
    auth: authResult,
  };
}
