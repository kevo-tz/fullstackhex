export const prerender = false;

export async function GET() {
    const response = await fetch(`${import.meta.env.VITE_RUST_BACKEND_URL}/health`);
    const body = await response.json();

    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    });
}
