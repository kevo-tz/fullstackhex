/// Typed feature flag helper.
///
/// Reads flags from the `/api/health` endpoint's `feature_flags` object.
/// All flags default to `false` when not present.

export interface FeatureFlags {
  chat_enabled: boolean;
  storage_readonly: boolean;
  maintenance_mode: boolean;
}

/** Parse feature flags from a health response object. */
export function parseFlags(data: Record<string, unknown>): FeatureFlags {
  const ff = data.feature_flags as Record<string, unknown> | undefined;
  return {
    chat_enabled: !!ff?.chat_enabled,
    storage_readonly: !!ff?.storage_readonly,
    maintenance_mode: !!ff?.maintenance_mode,
  };
}
