import { describe, it, expect } from "vitest";
import {
  fetchFeatureFlags,
  isFeatureEnabled,
  getEnabledFeatures,
  type FeatureFlags,
} from "../src/lib/flags";

describe("fetchFeatureFlags", () => {
  it("parses flags from health response", async () => {
    const mockFetch = async () =>
      new Response(
        JSON.stringify({
          rust: { status: "ok" },
          feature_flags: {
            chat_enabled: true,
            storage_readonly: false,
            maintenance_mode: true,
          },
        }),
        { status: 200 },
      );

    const flags = await fetchFeatureFlags(mockFetch as unknown as typeof fetch);
    expect(flags).toEqual({
      chat_enabled: true,
      storage_readonly: false,
      maintenance_mode: true,
    });
  });

  it("returns null on non-ok response", async () => {
    const mockFetch = async () => new Response(null, { status: 500 });
    const flags = await fetchFeatureFlags(mockFetch as unknown as typeof fetch);
    expect(flags).toBeNull();
  });

  it("returns null on fetch error", async () => {
    const mockFetch = async () => { throw new Error("network error"); };
    const flags = await fetchFeatureFlags(mockFetch as unknown as typeof fetch);
    expect(flags).toBeNull();
  });

  it("returns null when feature_flags field is absent", async () => {
    const mockFetch = async () =>
      new Response(JSON.stringify({ rust: { status: "ok" } }), { status: 200 });

    const flags = await fetchFeatureFlags(mockFetch as unknown as typeof fetch);
    expect(flags).toBeNull();
  });
});

describe("isFeatureEnabled", () => {
  const flags: FeatureFlags = {
    chat_enabled: true,
    storage_readonly: false,
    maintenance_mode: true,
  };

  it("returns true for enabled flag", () => {
    expect(isFeatureEnabled(flags, "chat_enabled")).toBe(true);
    expect(isFeatureEnabled(flags, "maintenance_mode")).toBe(true);
  });

  it("returns false for disabled flag", () => {
    expect(isFeatureEnabled(flags, "storage_readonly")).toBe(false);
  });

  it("returns false when flags is null", () => {
    expect(isFeatureEnabled(null, "chat_enabled")).toBe(false);
    expect(isFeatureEnabled(null, "storage_readonly")).toBe(false);
    expect(isFeatureEnabled(null, "maintenance_mode")).toBe(false);
  });
});

describe("getEnabledFeatures", () => {
  const flags: FeatureFlags = {
    chat_enabled: true,
    storage_readonly: false,
    maintenance_mode: true,
  };

  it("returns keys of enabled flags", () => {
    const enabled = getEnabledFeatures(flags);
    expect(enabled).toContain("chat_enabled");
    expect(enabled).toContain("maintenance_mode");
    expect(enabled).not.toContain("storage_readonly");
  });

  it("returns empty array when flags is null", () => {
    expect(getEnabledFeatures(null)).toEqual([]);
  });

  it("returns empty array when all flags are disabled", () => {
    const allOff: FeatureFlags = {
      chat_enabled: false,
      storage_readonly: false,
      maintenance_mode: false,
    };
    expect(getEnabledFeatures(allOff)).toEqual([]);
  });
});
