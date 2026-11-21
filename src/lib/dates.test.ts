import { describe, expect, it } from "vitest";
import { formatDate, todayIso } from "./dates";
import { addMonths, formatMonth, thisMonth } from "./dates";

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

describe("months", () => {
  it("formats, steps across years and reads this month locally", () => {
    expect(formatMonth("2026-11")).toBe("November 2026");
    expect(formatMonth("2026-01", true)).toBe("Jan 2026");
    expect(formatMonth("2026-13")).toBe("2026-13");
    expect(addMonths("2026-12", 1)).toBe("2027-01");
    expect(addMonths("2026-01", -1)).toBe("2025-12");
    expect(addMonths("2026-11", -23)).toBe("2024-12");
    expect(thisMonth(new Date(2026, 10, 30, 23, 59))).toBe("2026-11");
  });
});
