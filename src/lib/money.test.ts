import { describe, expect, it } from "vitest";
import { formatCents, MAX_CENTS, plainCents } from "./money";

// These cases mirror `display_format` / `plain_format` in
// src-tauri/core/src/money.rs. If one side changes, change both.
describe("formatCents", () => {
  it.each([
    [0, "$0.00"],
    [5, "$0.05"],
    [-5, "-$0.05"],
    [100, "$1.00"],
    [99_999, "$999.99"],
    [100_000, "$1,000.00"],
    [123_456_789, "$1,234,567.89"],
    [-123_456, "-$1,234.56"],
    [MAX_CENTS, "$90,071,992,547,409.91"],
    [-MAX_CENTS, "-$90,071,992,547,409.91"],
  ])("%d -> %s", (cents, expected) => {
    expect(formatCents(cents)).toBe(expected);
  });

  it("never prints negative zero", () => {
    expect(formatCents(-0)).toBe("$0.00");
  });

  it.each([1.5, NaN, Infinity, MAX_CENTS + 1])("rejects %d", (bad) => {
    expect(() => formatCents(bad)).toThrow(RangeError);
  });
});

describe("plainCents", () => {
  it.each([
    [0, "0.00"],
    [123_456, "1234.56"],
    [-5, "-0.05"],
  ])("%d -> %s", (cents, expected) => {
    expect(plainCents(cents)).toBe(expected);
  });
});
