import { expect, test } from "bun:test";

test("generated frontend smoke test", () => {
  expect(typeof Bun.version).toBe("string");
});
