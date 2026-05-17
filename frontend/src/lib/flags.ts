/// <reference types="astro/client" />

import type { HealthResponse } from "./health";

export interface FeatureFlags {
  chat_enabled: boolean;
  storage_readonly: boolean;
  maintenance_mode: boolean;
}

// Re-export for convenience
export type { HealthResponse };

/**
 * Fetch feature flags from the health endpoint.
 * Returns null on failure or if flags are absent.
 */
export async function fetchFeatureFlags(
  fetchImpl: typeof fetch = fetch,
): Promise<FeatureFlags | null> {
  try {
    const res = await fetchImpl("/api/health");
    if (!res.ok) return null;
    const data: HealthResponse = await res.json();
    return data.feature_flags ?? null;
  } catch {
    return null;
  }
}

/**
 * Safe check whether a specific feature flag is enabled.
 * Returns false when flags object is null.
 */
export function isFeatureEnabled(
  flags: FeatureFlags | null,
  key: keyof FeatureFlags,
): boolean {
  return flags?.[key] ?? false;
}

/**
 * Returns array of feature flag keys that are enabled.
 */
export function getEnabledFeatures(
  flags: FeatureFlags | null,
): (keyof FeatureFlags)[] {
  if (!flags) return [];
  return (Object.keys(flags) as (keyof FeatureFlags)[]).filter(
    (k) => flags[k],
  );
}
