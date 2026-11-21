import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { axisLabel, lineBounds, niceMax } from "../components/Charts";
import { api, Report } from "../lib/api";
import { rangeMonths, ReportsPage } from "./ReportsPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return { ...real, api: { ...real.api, report: vi.fn(), listCategories: vi.fn() } };
});
const m = vi.mocked(api);

function report(from: string, to: string): Report {
  return {
    from,
    to,
    months: [
      { month: "2026-10", income: 600_000, spending: 410_000, net: 190_000, net_worth: -47_000_000 },
      { month: "2026-11", income: 600_000, spending: 245_050, net: 354_950, net_worth: -46_603_383 },
    ],
    spending_by_category: [
      { category_id: 10, spent: 75_000 },
      { category_id: null, spent: 25_000 },
    ],
    totals: { income: 1_200_000, spending: 655_050, net: 544_950 },
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  m.report.mockImplementation(async (from, to) => report(from, to));
  m.listCategories.mockResolvedValue([
    {
      id: 2, name: "Food", kind: "expense", sort_order: 2, archived_at: null,
      categories: [{ id: 10, group_id: 2, name: "Groceries", sort_order: 1, archived_at: null }],
    },
  ]);
});

describe("chart helpers", () => {
  it("nice axis maxima and compact labels", () => {
    expect([niceMax(0), niceMax(7), niceMax(130), niceMax(4_100_000)]).toEqual([1, 10, 200, 5_000_000]);
    expect(axisLabel(0)).toBe("$0");
    expect(axisLabel(25_000)).toBe("$250");
    expect(axisLabel(250_000)).toBe("$2.5k");
    expect(axisLabel(-50_000_000)).toBe("-$500k");
    expect(axisLabel(120_000_000)).toBe("$1.2m");
  });

  it("line bounds hug the data instead of forcing zero", () => {
    const [lo, hi, ticks] = lineBounds([-47_000_000, -46_600_000]);
    expect(lo).toBeLessThanOrEqual(-47_000_000);
    expect(hi).toBeGreaterThanOrEqual(-46_600_000);
    expect(hi).toBeLessThan(0);
    expect(hi - lo).toBeLessThan(2_000_000);
    expect(ticks[0]).toBe(lo);
    expect(ticks[ticks.length - 1]).toBe(hi);
    const [flatLo, flatHi] = lineBounds([5_000, 5_000]);
    expect(flatLo).toBeLessThan(5_000);
    expect(flatHi).toBeGreaterThan(5_000);
  });

  it("ranges are inclusive month spans", () => {
    expect(rangeMonths("6m", "2026-11")).toEqual(["2026-06", "2026-11"]);
    expect(rangeMonths("12m", "2026-03")).toEqual(["2025-04", "2026-03"]);
    expect(rangeMonths("ytd", "2026-11")).toEqual(["2026-01", "2026-11"]);
    expect(rangeMonths("last-year", "2026-11")).toEqual(["2025-01", "2025-12"]);
  });
});

describe("ReportsPage", () => {
  it("loads the last 6 months and shows totals, charts and categories", async () => {
    render(<ReportsPage now="2026-11" />);
    expect(await screen.findByText("$5,449.50")).toBeInTheDocument();
    expect(m.report).toHaveBeenCalledWith("2026-06", "2026-11");
    expect(screen.getByText("June 2026 – November 2026")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Income and spending by month" })).toBeInTheDocument();
    const nw = screen.getByRole("img", { name: "Net worth at the end of each month" });
    expect(nw.querySelectorAll("circle")).toHaveLength(2);
    // Screen-reader tables carry the numbers.
    const table = screen.getByRole("table", { name: "Income and spending by month" });
    expect(within(table).getByRole("row", { name: /Nov ’26/ })).toHaveTextContent("$6,000.00$2,450.50");
    const cats = within(screen.getByRole("region", { name: "Spending by category" })).getAllByRole("listitem");
    expect(cats[0]).toHaveTextContent("Groceries75%$750.00");
    expect(cats[1]).toHaveTextContent("Uncategorised25%$250.00");
  });

  it("changing the period reloads", async () => {
    const user = userEvent.setup();
    render(<ReportsPage now="2026-11" />);
    await screen.findByText("$5,449.50");
    await user.selectOptions(screen.getByLabelText("Period"), "last-year");
    expect(m.report).toHaveBeenLastCalledWith("2025-01", "2025-12");
  });

  it("errors offer a retry", async () => {
    m.report.mockRejectedValueOnce({ code: "invalid", field: "to", message: "Reports cover at most 10 years." });
    render(<ReportsPage now="2026-11" />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Reports cover at most 10 years.");
  });
});
