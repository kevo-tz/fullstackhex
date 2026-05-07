import { describe, expect, test, beforeEach, afterEach, vi } from "vitest";

const TOKEN_KEY = "fullstackhex_token";
const USER_KEY = "fullstackhex_user";
const REFRESH_KEY = "fullstackhex_refresh_token";

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

function renderDashboard(): void {
  const token = localStorage.getItem(TOKEN_KEY);
  const content = document.getElementById("dashboard-content")!;
  const guard = document.getElementById("auth-guard")!;

  if (!token) {
    guard.style.display = "block";
    content.style.display = "none";
  } else {
    content.style.display = "block";
    guard.style.display = "none";

    try {
      const user = JSON.parse(localStorage.getItem(USER_KEY) || "{}");
      document.getElementById("user-email")!.textContent = user.email || "\u2014";
      document.getElementById("user-name")!.textContent = user.name || "\u2014";
      document.getElementById("user-provider")!.textContent = user.provider || "email";
    } catch {
      console.warn("Failed to parse user data from localStorage");
    }
  }
}

describe("Dashboard auth gating", () => {
  beforeEach(() => {
    document.body.innerHTML = buildDashboardHTML();
    localStorage.clear();
  });

  test("shows auth guard when no token", () => {
    renderDashboard();

    expect(document.getElementById("dashboard-content")!.style.display).toBe("none");
    expect(document.getElementById("auth-guard")!.style.display).toBe("block");
  });

  test("shows dashboard content when token exists", () => {
    localStorage.setItem(TOKEN_KEY, "valid-token");
    localStorage.setItem(
      USER_KEY,
      JSON.stringify({ email: "test@example.com", name: "Test", provider: "email" }),
    );

    renderDashboard();

    expect(document.getElementById("dashboard-content")!.style.display).toBe("block");
    expect(document.getElementById("auth-guard")!.style.display).toBe("none");
  });

  test("displays user info from localStorage", () => {
    localStorage.setItem(TOKEN_KEY, "valid-token");
    localStorage.setItem(
      USER_KEY,
      JSON.stringify({ email: "a@b.com", name: "Alice", provider: "google" }),
    );

    renderDashboard();

    expect(document.getElementById("user-email")!.textContent).toBe("a@b.com");
    expect(document.getElementById("user-name")!.textContent).toBe("Alice");
    expect(document.getElementById("user-provider")!.textContent).toBe("google");
  });

  test("falls back to dash on missing user fields", () => {
    localStorage.setItem(TOKEN_KEY, "valid-token");
    localStorage.setItem(USER_KEY, JSON.stringify({}));

    renderDashboard();

    expect(document.getElementById("user-email")!.textContent).toBe("\u2014");
    expect(document.getElementById("user-name")!.textContent).toBe("\u2014");
    expect(document.getElementById("user-provider")!.textContent).toBe("email");
  });

  test("falls back on corrupted user JSON", () => {
    localStorage.setItem(TOKEN_KEY, "valid-token");
    localStorage.setItem(USER_KEY, "not-json");

    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    renderDashboard();

    expect(document.getElementById("user-email")!.textContent).toBe("");
    expect(warn).toHaveBeenCalledWith("Failed to parse user data from localStorage");

    warn.mockRestore();
  });
});

describe("Token refresh interceptor", () => {
  let origFetch: typeof globalThis.fetch;
  let backend: ReturnType<typeof vi.fn>;

  function interceptedFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
    return (async () => {
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
    origFetch = vi.fn();
    backend = origFetch as unknown as ReturnType<typeof vi.fn>;
    localStorage.clear();
  });

  test("passes through non-401 responses", async () => {
    backend.mockResolvedValue(new Response("ok", { status: 200 }));

    const res = await interceptedFetch("/api/test");

    expect(res.status).toBe(200);
    expect(backend).toHaveBeenCalledTimes(1);
  });

  test("returns 401 when no refresh token available", async () => {
    backend.mockResolvedValue(new Response("unauthorized", { status: 401 }));

    const res = await interceptedFetch("/api/test");

    expect(res.status).toBe(401);
  });

  test("attempts refresh on 401 with refresh token", async () => {
    localStorage.setItem(REFRESH_KEY, "valid-refresh");
    backend
      .mockResolvedValueOnce(new Response("unauthorized", { status: 401 }))
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            access_token: "new-access",
            refresh_token: "new-refresh",
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(new Response("retried data", { status: 200 }));

    const res = await interceptedFetch("/api/test");

    expect(res.status).toBe(200);
    expect(backend).toHaveBeenCalledTimes(3);
    expect(localStorage.getItem(TOKEN_KEY)).toBe("new-access");
    expect(localStorage.getItem(REFRESH_KEY)).toBe("new-refresh");
  });

  test("clears auth and redirects on refresh rejection", async () => {
    localStorage.setItem(TOKEN_KEY, "old-token");
    localStorage.setItem(USER_KEY, JSON.stringify({ email: "a@b.com" }));
    localStorage.setItem(REFRESH_KEY, "expired-refresh");
    backend
      .mockResolvedValueOnce(new Response("unauthorized", { status: 401 }))
      .mockResolvedValueOnce(new Response("unauthorized", { status: 401 }));

    const originalLocation = window.location.href;
    Object.defineProperty(window, "location", {
      value: { href: "/dashboard" },
      writable: true,
    });

    await interceptedFetch("/api/test");

    expect(localStorage.getItem(TOKEN_KEY)).toBeNull();
    expect(localStorage.getItem(USER_KEY)).toBeNull();
    expect(localStorage.getItem(REFRESH_KEY)).toBeNull();
    expect(window.location.href).toBe("/login");

    Object.defineProperty(window, "location", {
      value: { href: originalLocation },
      writable: true,
    });
  });
});
