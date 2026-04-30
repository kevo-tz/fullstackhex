export const prerender = false;

import type { APIRoute } from "astro";

export const GET: APIRoute = async () => {
    const apiBase = import.meta.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";

    const result: Record<string, unknown> = {
        rust: { status: "unknown" },
        db: { status: "unknown" },
        python: { status: "unknown" },
    };

    try {
        const r = await fetch(`${apiBase}/health`);
        const d = await r.json();
        result.rust = { status: d.status ?? "unknown" };
    } catch {
        result.rust = { status: "error" };
    }

    try {
        const r = await fetch(`${apiBase}/health/db`);
        result.db = await r.json();
    } catch {
        result.db = { status: "error" };
    }

    try {
        const r = await fetch(`${apiBase}/health/python`);
        result.python = await r.json();
    } catch {
        result.python = { status: "unavailable" };
    }

    return new Response(JSON.stringify(result), {
        headers: { "Content-Type": "application/json" },
    });
};
