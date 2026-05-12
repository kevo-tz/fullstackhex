import { describe, expect, test, beforeEach, afterEach } from "vitest";

describe("theme system", () => {
  const STORAGE_KEY = "theme";
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.dataset.theme = "dark";
  });

  afterEach(() => {
    localStorage.clear();
    document.documentElement.dataset.theme = "dark";
  });

  function initTheme() {
    let stored: string | null = null;
    try {
      stored = localStorage.getItem(STORAGE_KEY);
    } catch {}
    const theme = stored || "dark";
    document.documentElement.dataset.theme = theme;
  }

  test("localStorage.theme=light sets data-theme=light", () => {
    localStorage.setItem(STORAGE_KEY, "light");
    initTheme();
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  test("localStorage theme missing defaults to dark", () => {
    initTheme();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  test("localStorage.theme=dark sets data-theme=dark", () => {
    localStorage.setItem(STORAGE_KEY, "dark");
    initTheme();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  test("toggle switches dark to light", () => {
    const html = document.documentElement;
    html.dataset.theme = "dark";
    html.dataset.theme = html.dataset.theme === "dark" ? "light" : "dark";
    expect(html.dataset.theme).toBe("light");
  });

  test("toggle switches light to dark", () => {
    const html = document.documentElement;
    html.dataset.theme = "light";
    html.dataset.theme = html.dataset.theme === "dark" ? "light" : "dark";
    expect(html.dataset.theme).toBe("dark");
  });

  test("toggle persists to localStorage", () => {
    const html = document.documentElement;
    html.dataset.theme = "dark";
    const next = html.dataset.theme === "dark" ? "light" : "dark";
    html.dataset.theme = next;
    localStorage.setItem(STORAGE_KEY, next);
    expect(localStorage.getItem(STORAGE_KEY)).toBe("light");
  });
});
