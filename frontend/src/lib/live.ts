/// <reference types="astro/client" />

export type LiveEvent =
  | { type: "health_update"; data: { service: string; status: string; detail?: string } }
  | { type: "auth_event"; data: { kind: string; email: string } }
  | { type: "connection_status"; data: { status: "connected" | "reconnecting" | "offline" } };

export interface LiveStream {
  onEvent(cb: (event: LiveEvent) => void): void;
  close(): void;
}

const MAX_RETRIES = 10;
const INITIAL_BACKOFF_MS = 500;
const MAX_BACKOFF_MS = 30_000;

/**
 * Calculate exponential backoff with full jitter for reconnection delay.
 * Returns a value in [0, cap] where cap doubles each attempt.
 */
function backoffForAttempt(attempt: number): number {
  const cap = Math.min(INITIAL_BACKOFF_MS * Math.pow(2, attempt - 1), MAX_BACKOFF_MS);
  return Math.random() * cap;
}

/**
 * Connect to the live WebSocket endpoint, returning a LiveStream handle.
 *
 * The WS connects to `/api/live` (proxied through Astro/Vite dev server to the
 * Rust backend). If the connection fails or the endpoint returns 404 (Redis
 * disabled), the caller should fall back to HTTP polling.
 *
 * Auto-reconnect: exponential backoff with jitter, max 10 retries.
 * On reconnect: creates a brand new connection (full re-subscribe).
 */
export function connectLiveStream(): LiveStream {
  const listeners: Array<(event: LiveEvent) => void> = [];
  let ws: WebSocket | null = null;
  let closed = false;
  let retryCount = 0;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;

  function emit(event: LiveEvent): void {
    for (const cb of listeners) {
      try {
        cb(event);
      } catch (e) {
        console.warn("[live] event handler error:", e);
      }
    }
  }

  function notifyStatus(status: "connected" | "reconnecting" | "offline"): void {
    emit({ type: "connection_status", data: { status } });
  }

  function scheduleReconnect(): void {
    if (closed) return;
    if (retryCount >= MAX_RETRIES) {
      console.warn(`[live] max retries (${MAX_RETRIES}) reached — giving up`);
      notifyStatus("offline");
      return;
    }

    retryCount++;
    const delay = backoffForAttempt(retryCount);
    notifyStatus("reconnecting");

    retryTimer = setTimeout(() => {
      retryTimer = null;
      connect();
    }, delay);
  }

  function connect(): void {
    if (closed) return;

    // Determine WS URL from current page location
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${location.host}/api/live`;

    try {
      ws = new WebSocket(url);
    } catch (e) {
      console.warn("[live] WebSocket constructor failed:", e);
      scheduleReconnect();
      return;
    }

    ws.onopen = () => {
      retryCount = 0;
      notifyStatus("connected");
    };

    ws.onmessage = (event: MessageEvent) => {
      let parsed: LiveEvent;
      try {
        parsed = JSON.parse(event.data as string) as LiveEvent;
      } catch {
        console.warn("[live] malformed JSON received — ignoring");
        return;
      }

      if (!parsed || typeof parsed.type !== "string") {
        console.warn("[live] invalid event shape — ignoring");
        return;
      }

      emit(parsed);
    };

    ws.onclose = () => {
      ws = null;
      if (!closed) {
        scheduleReconnect();
      }
    };

    ws.onerror = () => {
      // onerror is always followed by onclose, so just log here
      console.warn("[live] WebSocket error — will attempt reconnect");
    };
  }

  function close(): void {
    closed = true;
    if (retryTimer !== null) {
      clearTimeout(retryTimer);
      retryTimer = null;
    }
    if (ws !== null) {
      ws.onclose = null; // prevent reconnect on intentional close
      ws.close();
      ws = null;
    }
    listeners.length = 0;
  }

  return {
    onEvent(cb: (event: LiveEvent) => void): void {
      listeners.push(cb);
    },
    close,
  };
}
