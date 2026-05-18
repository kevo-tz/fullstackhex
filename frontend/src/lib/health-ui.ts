export function setStatus(id: string, status: string, label: string): void {
  const dot = document.getElementById("dot-" + id);
  const text = document.getElementById("text-" + id);
  if (dot) dot.className = "dot " + (status || "loading");
  if (text) {
    text.textContent = label || status || "…";
    const sr = document.getElementById("sr-" + id);
    if (sr) sr.textContent = id + ": " + (label || status || "…");
  }
}

export function setDetail(id: string, text: string): void {
  const el = document.getElementById("detail-" + id);
  if (el) el.textContent = text || "";
}

export function setDiagnostic(id: string, text: string): void {
  const el = document.getElementById("diag-" + id);
  if (!el) return;
  if (text) {
    el.textContent = text;
    el.classList.add("visible");
  } else {
    el.textContent = "";
    el.classList.remove("visible");
  }
}

export function showOutage(visible: boolean): void {
  const el = document.getElementById("outage-block");
  if (el) el.classList.toggle("visible", visible);
}
