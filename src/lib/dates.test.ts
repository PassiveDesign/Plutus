import { describe, expect, it } from "vitest";
import { formatDate, todayIso } from "./dates";

describe("dates", () => {
  it.each([
    ["2026-11-05", "5 Nov 2026"],
    ["2026-01-31", "31 Jan 2026"],
    ["2026-12-01", "1 Dec 2026"],
    ["2026-13-01", "2026-13-01"],
    ["nonsense", "nonsense"],
  ])("formatDate(%s) = %s", (input, out) => expect(formatDate(input)).toBe(out));

  it("todayIso uses local calendar fields", () => {
    expect(todayIso(new Date(2026, 0, 9, 23, 59))).toBe("2026-01-09");
  });
});
