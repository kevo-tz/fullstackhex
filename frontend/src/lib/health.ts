export async function aggregateHealth(
  fetchImpl: typeof fetch,
  apiBase = "http://localhost:8001",
): Promise<Record<string, unknown>> {
  const result: Record<string, unknown> = {
    rust: { status: "unknown" },
    db: { status: "unknown" },
    python: { status: "unknown" },
  };

  const [rustRes, dbRes, pythonRes] = await Promise.allSettled([
    fetchImpl(`${apiBase}/health`),
    fetchImpl(`${apiBase}/health/db`),
    fetchImpl(`${apiBase}/health/python`),
  ]);

  if (rustRes.status === "fulfilled") {
    try {
      const d = await rustRes.value.json();
      result.rust = { status: (d as Record<string, unknown>).status ?? "unknown" };
    } catch {
      result.rust = { status: "error" };
    }
  } else {
    result.rust = { status: "error" };
  }

  if (dbRes.status === "fulfilled") {
    try {
      result.db = await dbRes.value.json();
    } catch {
      result.db = { status: "error" };
    }
  } else {
    result.db = { status: "error" };
  }

  if (pythonRes.status === "fulfilled") {
    try {
      result.python = await pythonRes.value.json();
    } catch {
      result.python = { status: "unavailable" };
    }
  } else {
    result.python = { status: "unavailable" };
  }

  return result;
}
