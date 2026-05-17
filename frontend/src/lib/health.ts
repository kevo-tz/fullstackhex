export const SERVICE_IDS = ["rust", "db", "redis", "storage", "python", "auth"] as const;
export type ServiceId = (typeof SERVICE_IDS)[number];

export const SERVICE_LABELS: Record<ServiceId, string> = {
  rust: "Rust API",
  db: "PostgreSQL",
  redis: "Redis",
  storage: "RustFS Storage",
  python: "Python sidecar",
  auth: "Auth",
};

export interface HealthEntry {
  status: string;
  error?: string;
  fix?: string;
  detail?: unknown;
}

export interface HealthResponse {
  rust: HealthEntry;
  db: HealthEntry;
  redis: HealthEntry;
  storage: HealthEntry;
  python: HealthEntry;
  auth: HealthEntry;
  feature_flags?: Record<string, boolean>;
}

function jsonLog(obj: Record<string, unknown>): void {
  if (typeof window === "undefined" && import.meta.env.DEV) {
    console.log(JSON.stringify(obj));
  }
}

export function isFullOutage(data: Record<string, unknown>): boolean {
  for (const svc of SERVICE_IDS) {
    const entry = data[svc] as Record<string, unknown> | undefined;
    if (entry?.status === "ok") return false;
  }
  return true;
}

export function getDiagnostics(
  data: Record<string, unknown>,
): { service: string; status: string; fix: string | null }[] {
  const result: { service: string; status: string; fix: string | null }[] = [];
  for (const svc of SERVICE_IDS) {
    const entry = data[svc] as Record<string, unknown> | undefined;
    if (!entry || entry.status === "ok") continue;
    result.push({
      service: SERVICE_LABELS[svc] || svc,
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

// Dev fallback; production uses VITE_RUST_BACKEND_URL env var
export const API_BASE = import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";

export async function aggregateHealth(
  fetchImpl: typeof fetch,
  apiBase = API_BASE,
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
    (async (): Promise<{ status: string }> => {
      try {
        const res = await fetchImpl(`${apiBase}/health`, {
          headers: { "x-trace-id": traceId },
        });
        const d = await res.json();
        const healthData = d as Record<string, unknown>;
        const rustHealth = healthData.rust as Record<string, unknown> | undefined;
        return { status: String(rustHealth?.status ?? "error") };
      } catch {
        return { status: "error" };
      }
    })(),

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