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

export interface RetryControllerOptions {
  /** Maximum number of attempts before giving up (default: no cap). */
  maxAttempts?: number;
  /** Called once when maxAttempts is reached and retrying stops. */
  onGiveUp?: () => void;
}

export function createRetryController(
  onRetry: () => void,
  maxDelay = 30000,
  initialDelay = 1000,
  options: RetryControllerOptions = {},
): { start: () => void; cancel: () => void; reset: () => void } {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let delay = initialDelay;
  let attempts = 0;

  function schedule() {
    timer = setTimeout(() => {
      onRetry();
      attempts++;
      if (options.maxAttempts !== undefined && attempts >= options.maxAttempts) {
        timer = null;
        if (options.onGiveUp) options.onGiveUp();
        return;
      }
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
    attempts = 0;
  }

  function doStart() {
    doCancel();
    attempts = 0;
    schedule();
  }

  return { start: doStart, cancel: doCancel, reset: doReset };
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
    message: "health check",
    trace_id: traceId,
  });

  try {
    const res = await fetchImpl(`${apiBase}/health`, {
      headers: { "x-trace-id": traceId },
    });
    const d = (await res.json()) as Record<string, unknown>;

    const result: Record<string, unknown> = {};
    for (const svc of SERVICE_IDS) {
      const entry = d[svc] as Record<string, unknown> | undefined;
      result[svc] = { status: String(entry?.status ?? (svc === "auth" ? "disabled" : "error")) };
    }
    if (d.feature_flags) {
      result.feature_flags = d.feature_flags as Record<string, boolean>;
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
  } catch {
    const durationMs = Math.round(performance.now() - start);
    jsonLog({
      timestamp: new Date().toISOString(),
      level: "warn",
      target: "frontend:health",
      message: "health check failed",
      trace_id: traceId,
      duration_ms: durationMs,
    });
    return {
      rust: { status: "error" },
      db: { status: "error" },
      redis: { status: "unavailable" },
      storage: { status: "unavailable" },
      python: { status: "unavailable" },
      auth: { status: "disabled" },
    };
  }
}