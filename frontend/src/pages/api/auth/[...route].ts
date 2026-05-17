export const prerender = false;

import type { APIRoute } from "astro";

// Dev fallback; production uses VITE_RUST_BACKEND_URL env var
const BACKEND =
  import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";

export const ALL: APIRoute = async ({ request, params }) => {
  const route = params.route || "";
  const url = `${BACKEND}/auth/${route}${new URL(request.url).search}`;

  const headers = new Headers();
  const ct = request.headers.get("content-type");
  if (ct) headers.set("content-type", ct);
  const auth = request.headers.get("authorization");
  if (auth) headers.set("authorization", auth);
  const cookie = request.headers.get("cookie");
  if (cookie) headers.set("cookie", cookie);
  const traceId = request.headers.get("x-trace-id");
  if (traceId) headers.set("x-trace-id", traceId);
  const xForwardedFor = request.headers.get("x-forwarded-for");
  if (xForwardedFor) headers.set("x-forwarded-for", xForwardedFor);

  const init: RequestInit = {
    method: request.method,
    headers,
  };

  if (request.method !== "GET" && request.method !== "HEAD") {
    init.body = await request.text();
  }

  try {
    const backendRes = await fetch(url, init);

    // Forward all backend response headers (security headers, content-type,
    // set-cookie, caching, etc.) so the auth proxy is fully transparent.
    // Using Headers() constructor which normalizes header names.
    const responseHeaders = new Headers(backendRes.headers);

    return new Response(backendRes.body, {
      status: backendRes.status,
      statusText: backendRes.statusText,
      headers: responseHeaders,
    });
  } catch {
    return new Response(JSON.stringify({ error: "Backend unreachable" }), {
      status: 502,
      headers: { "Content-Type": "application/json" },
    });
  }
};
