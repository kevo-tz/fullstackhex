import { describe, expect, test, beforeEach, vi } from "vitest";

function buildDashboardHTML(): string {
  return `<!DOCTYPE html>
<html><body>
<div id="dashboard-content" style="display:none">
  <span id="user-email"></span>
  <span id="user-name"></span>
  <span id="user-provider"></span>
  <button id="dashboard-logout">Log out</button>
</div>
<div class="guard-msg" id="auth-guard">
  <p>You need to be logged in</p>
  <a href="/login">Sign in</a>
</div>
</body></html>`;
}

async function fetchUser(): Promise<Record<string, string> | null> {
  try {
    const res = await fetch("/api/auth/me");
    if (!res.ok) return null;
    const data = await res.json();
    if (data?.status === "disabled") return null;
    return data;
  } catch {
    return null;
  }
}

async function renderDashboard(): Promise<void> {
  const content = document.getElementById("dashboard-content")!;
  const guard = document.getElementById("auth-guard")!;

  try {
    const user = await fetchUser();
    if (!user) {
      guard.style.display = "block";
      content.style.display = "none";
    } else {
      content.style.display = "block";
      guard.style.display = "none";
      document.getElementById("user-email")!.textContent = user.email || "\u2014";
      document.getElementById("user-name")!.textContent = user.name || "\u2014";
      document.getElementById("user-provider")!.textContent = user.provider || "email";
    }
  } catch {
    guard.style.display = "block";
    content.style.display = "none";
  }
}

describe("Dashboard auth gating", () => {
  beforeEach(() => {
    document.body.innerHTML = buildDashboardHTML();
  });

  test("shows auth guard when fetch responds with 401", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 401 })));
    await renderDashboard();
    expect(document.getElementById("dashboard-content")!.style.display).toBe("none");
    expect(document.getElementById("auth-guard")!.style.display).toBe("block");
    vi.unstubAllGlobals();
  });

  test("shows auth guard when fetch fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("network error")));
    await renderDashboard();
    expect(document.getElementById("dashboard-content")!.style.display).toBe("none");
    expect(document.getElementById("auth-guard")!.style.display).toBe("block");
    vi.unstubAllGlobals();
  });

  test("shows auth guard when auth is disabled", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ status: "disabled" }), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      ),
    );
    await renderDashboard();
    expect(document.getElementById("dashboard-content")!.style.display).toBe("none");
    expect(document.getElementById("auth-guard")!.style.display).toBe("block");
    vi.unstubAllGlobals();
  });

  test("shows dashboard content when authenticated", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            user_id: "u1",
            email: "test@example.com",
            name: "Test",
            provider: "local",
          }),
          {
            status: 200,
            headers: { "content-type": "application/json" },
          },
        ),
      ),
    );
    await renderDashboard();
    expect(document.getElementById("dashboard-content")!.style.display).toBe("block");
    expect(document.getElementById("auth-guard")!.style.display).toBe("none");
    vi.unstubAllGlobals();
  });

  test("displays user info from fetch response", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            user_id: "u2",
            email: "alice@example.com",
            name: "Alice",
            provider: "google",
          }),
          {
            status: 200,
            headers: { "content-type": "application/json" },
          },
        ),
      ),
    );
    await renderDashboard();
    expect(document.getElementById("user-email")!.textContent).toBe("alice@example.com");
    expect(document.getElementById("user-name")!.textContent).toBe("Alice");
    expect(document.getElementById("user-provider")!.textContent).toBe("google");
    vi.unstubAllGlobals();
  });

  test("falls back to dash on missing user fields", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            user_id: "u3",
            email: "bob@example.com",
            name: null,
            provider: "local",
          }),
          {
            status: 200,
            headers: { "content-type": "application/json" },
          },
        ),
      ),
    );
    await renderDashboard();
    expect(document.getElementById("user-name")!.textContent).toBe("\u2014");
    expect(document.getElementById("user-email")!.textContent).toBe("bob@example.com");
    vi.unstubAllGlobals();
  });
});

describe("Token refresh interceptor", () => {
  let origFetch: typeof globalThis.fetch;
  let backend: ReturnType<typeof vi.fn>;

  function interceptedFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
    return (async () => {
      const TOKEN_KEY = "fullstackhex_token";
      const REFRESH_KEY = "fullstackhex_refresh_token";
      const USER_KEY = "fullstackhex_user";

      const res = await origFetch(input, init);
      if (res.status !== 401) return res;
      const refreshToken = localStorage.getItem(REFRESH_KEY);
      if (!refreshToken) return res;
      try {
        const refreshRes = await origFetch("/api/auth/refresh", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ refresh_token: refreshToken }),
        });
        if (refreshRes.ok) {
          const data = await refreshRes.json();
          localStorage.setItem(TOKEN_KEY, data.access_token);
          if (data.refresh_token) localStorage.setItem(REFRESH_KEY, data.refresh_token);
          const newInit: RequestInit = { ...init };
          const newHeaders = new Headers(init?.headers);
          newHeaders.set("Authorization", `Bearer ${data.access_token}`);
          newInit.headers = newHeaders;
          return origFetch(input, newInit);
        }
        localStorage.removeItem(TOKEN_KEY);
        localStorage.removeItem(USER_KEY);
        localStorage.removeItem(REFRESH_KEY);
        window.location.href = "/login";
        return res;
      } catch {
        console.warn("Token refresh request failed");
        return res;
      }
    })();
  }

  beforeEach(() => {
    origFetch = vi.fn() as unknown as typeof globalThis.fetch;
    backend = origFetch as unknown as ReturnType<typeof vi.fn>;
    localStorage.clear();
  });

  test("passes through non-401 responses", async () => {
    backend.mockResolvedValue(new Response("ok", { status: 200 }));
    const res = await interceptedFetch("/api/notes");
    expect(res.status).toBe(200);
  });

  test("returns 401 when no refresh token available", async () => {
    backend.mockResolvedValue(new Response("unauthorized", { status: 401 }));
    const res = await interceptedFetch("/api/notes");
    expect(res.status).toBe(401);
  });

  test("attempts refresh on 401 with refresh token", async () => {
    localStorage.setItem("fullstackhex_refresh_token", "rt1");
    // First call: 401; refresh call: 200 with new tokens; retry: 200
    backend
      .mockResolvedValueOnce(new Response("unauthorized", { status: 401 }))
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            access_token: "at2",
            refresh_token: "rt2",
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));
    const res = await interceptedFetch("/api/notes");
    expect(res.status).toBe(200);
    expect(localStorage.getItem("fullstackhex_token")).toBe("at2");
    expect(localStorage.getItem("fullstackhex_refresh_token")).toBe("rt2");
  });

  test("clears auth and redirects on refresh rejection", async () => {
    localStorage.setItem("fullstackhex_token", "at1");
    localStorage.setItem("fullstackhex_refresh_token", "rt1");
    localStorage.setItem("fullstackhex_user", '{"email":"a@b.com"}');
    // First call: 401; refresh call: 401
    backend
      .mockResolvedValueOnce(new Response("unauthorized", { status: 401 }))
      .mockResolvedValueOnce(new Response("unauthorized", { status: 401 }));
    await interceptedFetch("/api/notes");
    expect(localStorage.getItem("fullstackhex_token")).toBeNull();
    expect(localStorage.getItem("fullstackhex_user")).toBeNull();
    expect(localStorage.getItem("fullstackhex_refresh_token")).toBeNull();
  });
});
