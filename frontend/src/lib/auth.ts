const TOKEN_KEY = "fullstackhex_token";
const REFRESH_KEY = "fullstackhex_refresh";
const USER_KEY = "fullstackhex_user";

export interface AuthUser {
  id: string;
  email: string;
  name: string | null;
  provider: string;
}

export interface AuthResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
  user: AuthUser;
}

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(TOKEN_KEY);
}

export function getRefreshToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(REFRESH_KEY);
}

export function getUser(): AuthUser | null {
  if (typeof window === "undefined") return null;
  const raw = localStorage.getItem(USER_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function isLoggedIn(): boolean {
  return getToken() !== null;
}

export function saveAuth(data: AuthResponse): void {
  localStorage.setItem(TOKEN_KEY, data.access_token);
  localStorage.setItem(REFRESH_KEY, data.refresh_token);
  localStorage.setItem(USER_KEY, JSON.stringify(data.user));
}

export function clearAuth(): void {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(REFRESH_KEY);
  localStorage.removeItem(USER_KEY);
}

let refreshing: Promise<AuthResponse | null> | null = null;

async function api(path: string, options: RequestInit = {}): Promise<Response> {
  const token = getToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string> | undefined),
  };
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }
  const res = await fetch(`/api${path}`, { ...options, headers });
  if (res.status === 401 && token) {
    const refreshed = await refreshToken();
    if (refreshed) {
      headers["Authorization"] = `Bearer ${refreshed.access_token}`;
      return fetch(`/api${path}`, { ...options, headers });
    }
    clearAuth();
  }
  return res;
}

export async function login(email: string, password: string): Promise<AuthResponse> {
  const res = await api("/auth/login", {
    method: "POST",
    body: JSON.stringify({ email, password }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: "Login failed" }));
    throw new Error(err.error || `Login failed (${res.status})`);
  }
  const data: AuthResponse = await res.json();
  saveAuth(data);
  return data;
}

export async function register(
  email: string,
  password: string,
  name?: string,
): Promise<AuthResponse> {
  const res = await api("/auth/register", {
    method: "POST",
    body: JSON.stringify({ email, password, name }),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: "Registration failed" }));
    throw new Error(err.error || `Registration failed (${res.status})`);
  }
  const data: AuthResponse = await res.json();
  saveAuth(data);
  return data;
}

export async function logout(): Promise<void> {
  try {
    await api("/auth/logout", { method: "POST" });
  } finally {
    clearAuth();
  }
}

export async function refreshToken(): Promise<AuthResponse | null> {
  const token = getRefreshToken();
  if (!token) return null;

  if (refreshing) return refreshing;

  refreshing = (async () => {
    try {
      const res = await fetch("/api/auth/refresh", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ refresh_token: token }),
      });
      if (!res.ok) {
        clearAuth();
        return null;
      }
      const data: AuthResponse = await res.json();
      saveAuth(data);
      return data;
    } catch {
      clearAuth();
      return null;
    } finally {
      refreshing = null;
    }
  })();

  return refreshing;
}

export async function fetchMe(): Promise<AuthUser | null> {
  const res = await api("/auth/me");
  if (!res.ok) return null;
  return res.json();
}
