import { describe, expect, test, beforeEach } from "vitest";

function buildDashboardHTML(): string {
  return `<!DOCTYPE html>
<html><body>
<div class="container">
  <div class="card" id="card-rust">
    <div class="card-label">Rust API</div>
    <div class="status-row">
      <div class="dot loading" id="dot-rust"></div>
      <span class="status-text" id="text-rust">checking…</span>
    </div>
  </div>
  <div class="card" id="card-db">
    <div class="card-label">PostgreSQL</div>
    <div class="status-row">
      <div class="dot loading" id="dot-db"></div>
      <span class="status-text" id="text-db">checking…</span>
    </div>
  </div>
  <div class="card" id="card-python">
    <div class="card-label">Python sidecar</div>
    <div class="status-row">
      <div class="dot loading" id="dot-python"></div>
      <span class="status-text" id="text-python">checking…</span>
    </div>
    <div class="detail" id="detail-python"></div>
  </div>
</div>
</body></html>`;
}

/**
 * Mirrors the setStatus function from index.astro inline script.
 * KEEP IN SYNC with the inline script in src/pages/index.astro.
 */
function setStatus(id: string, status: string, label: string): void {
  const dot = document.getElementById("dot-" + id);
  const text = document.getElementById("text-" + id);
  if (dot) dot.className = "dot " + (status || "loading");
  if (text) text.textContent = label || status || "…";
}

function setDetail(id: string, text: string): void {
  const el = document.getElementById("detail-" + id);
  if (el) el.textContent = text || "";
}

describe("Dashboard health status display", () => {
  beforeEach(() => {
    document.body.innerHTML = buildDashboardHTML();
  });

  describe("initial state", () => {
    test("all dots have loading class on page load", () => {
      const rust = document.getElementById("dot-rust")!;
      const db = document.getElementById("dot-db")!;
      const python = document.getElementById("dot-python")!;

      expect(rust.className).toContain("loading");
      expect(db.className).toContain("loading");
      expect(python.className).toContain("loading");
    });

    test("status texts show checking state", () => {
      expect(document.getElementById("text-rust")!.textContent).toBe(
        "checking…",
      );
      expect(document.getElementById("text-db")!.textContent).toBe("checking…");
      expect(document.getElementById("text-python")!.textContent).toBe(
        "checking…",
      );
    });
  });

  describe("all-green — all services healthy", () => {
    test("all dots show 'ok' class", () => {
      setStatus("rust", "ok", "ok");
      setStatus("db", "ok", "ok");
      setStatus("python", "ok", "ok");

      expect(document.getElementById("dot-rust")!.className).toContain("ok");
      expect(document.getElementById("dot-db")!.className).toContain("ok");
      expect(document.getElementById("dot-python")!.className).toContain("ok");
    });

    test("all status texts show 'ok'", () => {
      setStatus("rust", "ok", "ok");
      setStatus("db", "ok", "ok");
      setStatus("python", "ok", "ok");

      expect(document.getElementById("text-rust")!.textContent).toBe("ok");
      expect(document.getElementById("text-db")!.textContent).toBe("ok");
      expect(document.getElementById("text-python")!.textContent).toBe("ok");
    });

    test("no dot has error or degraded class", () => {
      setStatus("rust", "ok", "ok");
      setStatus("db", "ok", "ok");
      setStatus("python", "ok", "ok");

      const dots = document.querySelectorAll(".dot");
      dots.forEach((dot) => {
        expect(dot.className).not.toContain("error");
        expect(dot.className).not.toContain("degraded");
      });
    });
  });

  describe("mixed — one healthy, DB down, Python unavailable", () => {
    test("rust dot shows ok, db shows error, python shows degraded", () => {
      setStatus("rust", "ok", "ok");
      setStatus("db", "error", "error: connection failed");
      setStatus("python", "degraded", "not running");

      expect(document.getElementById("dot-rust")!.className).toContain("ok");
      expect(document.getElementById("dot-db")!.className).toContain("error");
      expect(document.getElementById("dot-python")!.className).toContain(
        "degraded",
      );
    });

    test("status texts reflect mixed state", () => {
      setStatus("rust", "ok", "ok");
      setStatus("db", "error", "error: connection failed");
      setStatus("python", "degraded", "not running");

      expect(document.getElementById("text-rust")!.textContent).toBe("ok");
      expect(document.getElementById("text-db")!.textContent).toBe(
        "error: connection failed",
      );
      expect(document.getElementById("text-python")!.textContent).toBe(
        "not running",
      );
    });

    test("python detail shows error text", () => {
      setStatus("python", "degraded", "not running");
      setDetail("python", "socket not found");

      expect(document.getElementById("detail-python")!.textContent).toBe(
        "socket not found",
      );
    });

    test("python detail clears when switching to ok", () => {
      setStatus("python", "degraded", "not running");
      setDetail("python", "socket not found");
      setStatus("python", "ok", "ok");
      setDetail("python", "");

      expect(document.getElementById("detail-python")!.textContent).toBe("");
    });
  });

  describe("all-red — API down", () => {
    test("all dots show error class", () => {
      setStatus("rust", "error", "unreachable");
      setStatus("db", "error", "—");
      setStatus("python", "error", "—");

      expect(document.getElementById("dot-rust")!.className).toContain("error");
      expect(document.getElementById("dot-db")!.className).toContain("error");
      expect(document.getElementById("dot-python")!.className).toContain(
        "error",
      );
    });

    test("status texts show error state", () => {
      setStatus("rust", "error", "unreachable");
      setStatus("db", "error", "—");
      setStatus("python", "error", "—");

      expect(document.getElementById("text-rust")!.textContent).toBe(
        "unreachable",
      );
      expect(document.getElementById("text-db")!.textContent).toBe("—");
      expect(document.getElementById("text-python")!.textContent).toBe("—");
    });
  });

  describe("CSS class transitions", () => {
    test("dot transitions from loading to ok", () => {
      const dot = document.getElementById("dot-rust")!;
      expect(dot.className).toContain("loading");

      setStatus("rust", "ok", "ok");
      expect(dot.className).toContain("ok");
      expect(dot.className).not.toContain("loading");
    });

    test("dot transitions from ok to error", () => {
      setStatus("rust", "ok", "ok");
      expect(document.getElementById("dot-rust")!.className).toContain("ok");

      setStatus("rust", "error", "unreachable");
      expect(document.getElementById("dot-rust")!.className).toContain("error");
      expect(document.getElementById("dot-rust")!.className).not.toContain(
        "ok",
      );
    });
  });
});
