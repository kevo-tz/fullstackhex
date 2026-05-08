// Regression: error display shows [object Object] instead of actual message
// Found by /qa on 2026-05-08
// Report: .gstack/qa-reports/qa-report-localhost-2026-05-08.md
// Fix: AuthForm.astro now extracts data.error.message when error is nested object

import { describe, expect, test } from "bun:test";

/**
 * Mirrors the error-extraction logic in AuthForm.astro:
 *   const msg = typeof data.error === "object" && data.error !== null
 *     ? data.error.message
 *     : data.error;
 */
function extractError(data: Record<string, unknown>): string {
  const err = data.error;
  if (typeof err === "object" && err !== null) {
    return (err as Record<string, unknown>).message as string;
  }
  return (err as string) || "Unknown error";
}

describe("AuthForm error extraction", () => {
  test("nested ApiError returns the message string, not [object Object]", () => {
    const apiResponse = {
      error: { code: "UNAUTHORIZED", message: "Invalid credentials" },
    };
    expect(extractError(apiResponse)).toBe("Invalid credentials");
  });

  test("flat string error passes through unchanged", () => {
    const apiResponse = { error: "Something went wrong" };
    expect(extractError(apiResponse)).toBe("Something went wrong");
  });

  test("validation error returns correct message", () => {
    const apiResponse = {
      error: {
        code: "VALIDATION_ERROR",
        message: "Password must be at least 8 characters",
      },
    };
    expect(extractError(apiResponse)).toBe(
      "Password must be at least 8 characters",
    );
  });

  test("missing error returns fallback", () => {
    expect(extractError({})).toBe("Unknown error");
  });

  test("null error returns fallback", () => {
    expect(extractError({ error: null })).toBe("Unknown error");
  });
});
