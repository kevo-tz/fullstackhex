export const prerender = false;

import type { APIRoute } from "astro";

const BACKEND =
  import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";

export const ALL: APIRoute = async ({ request, params }) => {
  const route = params.route || "";
  const url = `${BACKEND}/auth/${route}${new URL(request.url).search}`;

  const headers = new Headers();
  // Forward content-type and authorization headers
  const ct = request.headers.get("content-type");
  if (ct) headers.set("content-type", ct);
  const auth = request.headers.get("authorization");
  if (auth) headers.set("authorization", auth);

  const init: RequestInit = {
    method: request.method,
    headers,
  };

  if (request.method !== "GET" && request.method !== "HEAD") {
    init.body = await request.text();
  }

  try {
    const backendRes = await fetch(url, init);

    return new Response(backendRes.body, {
      status: backendRes.status,
      statusText: backendRes.statusText,
      headers: {
        "content-type":
          backendRes.headers.get("content-type") || "application/json",
      },
    });
  } catch {
    return new Response(JSON.stringify({ error: "Backend unreachable" }), {
      status: 502,
      headers: { "Content-Type": "application/json" },
    });
  }
};
