export const prerender = false;

import type { APIRoute } from "astro";
import { aggregateHealth } from "../../lib/health";

export const GET: APIRoute = async () => {
  const apiBase =
    import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";
  const result = await aggregateHealth(fetch, apiBase);

  return new Response(JSON.stringify(result), {
    headers: { "Content-Type": "application/json" },
  });
};
