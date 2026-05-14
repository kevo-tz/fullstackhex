import { expect, test } from "vitest";

test("generated frontend smoke test", () => {
  expect(typeof process.version).toBe("string");
});
