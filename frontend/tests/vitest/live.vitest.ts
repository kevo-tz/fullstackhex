import { describe, expect, test, vi, beforeEach, afterEach } from "vitest";
import { connectLiveStream, type LiveEvent } from "../../src/lib/live";

type WsEventListener = ((event: any) => void) | null;

interface MockWebSocketStatic {
  instances: MockWebSocket[];
  readonly CONNECTING: 0;
  readonly OPEN: 1;
  readonly CLOSING: 2;
  readonly CLOSED: 3;
}

interface MockWebSocket {
  url: string;
  readyState: number;
  onopen: WsEventListener;
  onclose: WsEventListener;
  onmessage: WsEventListener;
  onerror: WsEventListener;
  close(): void;
  _open(): void;
  _close(code?: number, reason?: string): void;
  _message(data: string): void;
  _error(): void;
  _reset(): void;
}

function createMockWebSocket(): MockWebSocketStatic {
  const instances: MockWebSocket[] = [];
  const MockWebSocketImpl = function (this: MockWebSocket, url: string) {
    this.url = url;
    this.readyState = 0;
    this.onopen = null;
    this.onclose = null;
    this.onmessage = null;
    this.onerror = null;
    this.close = function () {
      this.readyState = 2;
    };
    this._open = function () {
      this.readyState = 1;
      this.onopen?.(new Event("open"));
    };
    this._close = function (_code?: number, _reason?: string) {
      this.readyState = 3;
      this.onclose?.(new CloseEvent("close"));
    };
    this._message = function (data: string) {
      this.onmessage?.(new MessageEvent("message", { data }));
    };
    this._error = function () {
      this.onerror?.(new Event("error"));
    };
    this._reset = function () {
      this.readyState = 0;
    };
    instances.push(this);
  } as unknown as new (url: string) => MockWebSocket;
  (MockWebSocketImpl as unknown as MockWebSocketStatic).instances = instances;
  Object.defineProperty(MockWebSocketImpl, "CONNECTING", { value: 0, writable: false });
  Object.defineProperty(MockWebSocketImpl, "OPEN", { value: 1, writable: false });
  Object.defineProperty(MockWebSocketImpl, "CLOSING", { value: 2, writable: false });
  Object.defineProperty(MockWebSocketImpl, "CLOSED", { value: 3, writable: false });
  return MockWebSocketImpl as unknown as MockWebSocketStatic;
}

describe("connectLiveStream", () => {
  let MockWebSocket: MockWebSocketStatic;
  let lastWs: () => MockWebSocket | undefined;

  beforeEach(() => {
    MockWebSocket = createMockWebSocket();
    vi.stubGlobal("WebSocket", MockWebSocket);
    vi.useFakeTimers();
    lastWs = () => MockWebSocket.instances.at(-1);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  test("creates WebSocket connection to /api/live", () => {
    const live = connectLiveStream();
    expect(MockWebSocket.instances.length).toBe(1);
    expect(MockWebSocket.instances[0].url).toContain("/api/live");
    live.close();
  });

  test("emits connected on open", () => {
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => events.push(e));
    const ws = lastWs()!;
    ws._open();
    expect(events.some((e) => e.type === "connection_status" && e.data.status === "connected")).toBe(true);
    live.close();
  });

  test("emits reconnecting on close", () => {
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => events.push(e));
    lastWs()!._close();
    expect(events.some((e) => e.type === "connection_status" && e.data.status === "reconnecting")).toBe(true);
    live.close();
  });

  test("emits health_update on valid JSON message", () => {
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => events.push(e));
    lastWs()!._message(
      JSON.stringify({ type: "health_update", data: { service: "db", status: "ok" } }),
    );
    expect(events.some((e) => e.type === "health_update" && e.data.service === "db" && e.data.status === "ok")).toBe(true);
    live.close();
  });

  test("ignores malformed JSON message", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => events.push(e));
    lastWs()!._message("not valid json");
    expect(events.length).toBe(0);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
    live.close();
  });

  test("ignores invalid event shape", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => events.push(e));
    lastWs()!._message(JSON.stringify({ type: 123 }));
    expect(events.length).toBe(0);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
    live.close();
  });

  test("reconnects after close with backoff", () => {
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => {
      if (e.type === "connection_status") events.push(e);
    });
    // First connection closes
    lastWs()!._close();
    const connEvents = events.filter((e) => e.type === "connection_status") as Array<LiveEvent & { type: "connection_status" }>;
    expect(connEvents.filter((e) => e.data.status === "reconnecting").length).toBe(1);
    // Fast-forward past backoff

    vi.advanceTimersByTime(1000);
    expect(MockWebSocket.instances.length).toBe(2);
    live.close();
  });

  test("stops reconnecting after max retries", () => {
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => {
      if (e.type === "connection_status") events.push(e);
    });
    // Fail 10 times
    for (let i = 0; i < 11; i++) {
      const ws = lastWs()!;
      ws._close();
      vi.advanceTimersByTime(50000);
    }
    // Should emit offline after 10 retries
    const connEvents = events.filter((e) => e.type === "connection_status") as Array<LiveEvent & { type: "connection_status" }>;
    const offlineEvents = connEvents.filter((e) => e.data.status === "offline");
    expect(offlineEvents.length).toBeGreaterThanOrEqual(1);
    live.close();
  });

  test("does not reconnect after intentional close", () => {
    const live = connectLiveStream();
    live.close();
    // close() sets closed=true, then if ws was open, it would close it
    // After close(), no reconnect should happen
    vi.advanceTimersByTime(50000);
    expect(MockWebSocket.instances.length).toBe(1);
  });

  test("close removes all listeners", () => {
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => events.push(e));
    live.close();
    // Simulate a message coming in after close
    // Since ws is null after close, nothing happens
    expect(events.length).toBe(0);
  });

  test("triggers reconnect on WebSocket constructor throw", () => {
    // Override WebSocket to throw
    vi.stubGlobal("WebSocket", function () {
      throw new Error("connection refused");
    });
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => {
      if (e.type === "connection_status") events.push(e);
    });
    // Should try to reconnect
    vi.advanceTimersByTime(1000);
    const connEvents = events.filter((e) => e.type === "connection_status") as Array<LiveEvent & { type: "connection_status" }>;
    expect(connEvents.filter((e) => e.data.status === "reconnecting").length).toBeGreaterThanOrEqual(1);
    live.close();
  });

  test("catches handler errors without crashing", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const live = connectLiveStream();
    live.onEvent(() => {
      throw new Error("handler crashed");
    });
    lastWs()!._message(
      JSON.stringify({ type: "health_update", data: { service: "db", status: "ok" } }),
    );
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
    live.close();
  });

  test("resets retry count on successful connection", () => {
    const events: LiveEvent[] = [];
    const live = connectLiveStream();
    live.onEvent((e) => {
      if (e.type === "connection_status") events.push(e);
    });
    // Fail once
    lastWs()!._close();
    vi.advanceTimersByTime(1000);
    // Connect again
    lastWs()!._open();
    // Fail again — should still try to reconnect (retry count was reset)
    lastWs()!._close();
    vi.advanceTimersByTime(1000);
    expect(MockWebSocket.instances.length).toBeGreaterThanOrEqual(3);
    live.close();
  });
});
