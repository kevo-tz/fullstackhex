function jsonLog(obj: Record<string, unknown>): void {
  console.log(JSON.stringify(obj));
}

export interface DiagnosticEntry {
  service: string;
  status: string;
  fix: string | null;
}

export function isFullOutage(data: Record<string, unknown>): boolean {
  const services = ["rust", "db", "redis", "storage", "python", "auth"];
  for (const svc of services) {
    const entry = data[svc] as Record<string, unknown> | undefined;
    if (!entry || entry.status === "ok") return false;
  }
  return true;
}

export function getDiagnostics(data: Record<string, unknown>): DiagnosticEntry[] {
  const services = ["rust", "db", "redis", "storage", "python", "auth"];
  const labels: Record<string, string> = {
    rust: "Rust API", db: "PostgreSQL", redis: "Redis",
    storage: "RustFS Storage", python: "Python sidecar", auth: "Auth",
  };
  const result: DiagnosticEntry[] = [];
  for (const svc of services) {
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

  const [rustResult, dbResult, redisResult, storageResult, pythonResult, authResult] = await Promise.all([
    handleService(fetchImpl, `${apiBase}/health`, "rust", "api", "error", traceId),
    handleService(fetchImpl, `${apiBase}/health/db`, "db", "db", "error", traceId),
    handleService(fetchImpl, `${apiBase}/health/redis`, "redis", "redis", "unavailable", traceId),
    handleService(fetchImpl, `${apiBase}/health/storage`, "storage", "storage", "unavailable", traceId),
    handleService(fetchImpl, `${apiBase}/health/python`, "python", "python", "unavailable", traceId),
    handleService(fetchImpl, `${apiBase}/health/auth`, "auth", "auth", "disabled", traceId),
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
