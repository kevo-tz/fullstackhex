export const prerender = false;

import type { APIRoute } from "astro";
import { aggregateHealth } from "../../lib/health";

const TTL_MS = 2000;
const CACHE_CONTROL = "public, max-age=2, s-maxage=5";

function hash(input: string): string {
  let h = 5381;
  for (let i = 0; i < input.length; i++) {
    h = ((h << 5) + h + input.charCodeAt(i)) >>> 0;
  }
  return h.toString(36);
}

let cache: { timestamp: number; body: string; etag: string } | null = null;

export const GET: APIRoute = async ({ request }) => {
  const now = Date.now();

  if (!cache || now - cache.timestamp >= TTL_MS) {
    const apiBase =
      import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";
    const result = await aggregateHealth(fetch, apiBase);
    const body = JSON.stringify(result);
    cache = { timestamp: now, body, etag: `"${hash(body)}"` };
  }

  const ifNoneMatch = request.headers.get("if-none-match");
  if (ifNoneMatch && ifNoneMatch === cache.etag) {
    return new Response(null, {
      status: 304,
      headers: { "Cache-Control": CACHE_CONTROL, ETag: cache.etag },
    });
  }

  return new Response(cache.body, {
    headers: {
      "Content-Type": "application/json",
      "Cache-Control": CACHE_CONTROL,
      ETag: cache.etag,
    },
  });
};
