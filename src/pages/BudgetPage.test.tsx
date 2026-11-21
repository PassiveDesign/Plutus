import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { api, BudgetLine, CategoryGroup, MonthBudget } from "../lib/api";
import { BudgetPage } from "./BudgetPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      budgetMonth: vi.fn(),
      listCategories: vi.fn(),
      setBudget: vi.fn(),
      clearBudget: vi.fn(),
      setBudgetRollover: vi.fn(),
      parseMoney: vi.fn(),
    },
  };
});
const m = vi.mocked(api);

const TREE: CategoryGroup[] = [
  {
    id: 2, name: "Food", kind: "expense", sort_order: 2, archived_at: null,
    categories: [
      { id: 10, group_id: 2, name: "Groceries", sort_order: 1, archived_at: null },
      { id: 11, group_id: 2, name: "Eating out", sort_order: 2, archived_at: null },
    ],
  },
  {
    id: 3, name: "Transport", kind: "expense", sort_order: 3, archived_at: null,
    categories: [{ id: 20, group_id: 3, name: "Fuel", sort_order: 1, archived_at: null }],
  },
];

function line(over: Partial<BudgetLine> & { category_id: number; group_id: number }): BudgetLine {
  return { budgeted: null, spent: 0, remaining: null, set_in: null, carried: 0, rollover: false, ...over };
}

function budget(month: string): MonthBudget {
  return {
    month,
    lines: [
      line({ category_id: 10, group_id: 2, budgeted: 80_000, spent: 54_000, remaining: 26_000, set_in: "2026-09" }),
      line({ category_id: 11, group_id: 2, spent: 2_500 }),
      line({ category_id: 20, group_id: 3, budgeted: 10_000, spent: 14_000, remaining: -2_000, set_in: month, rollover: true, carried: 2_000 }),
    ],
    total_budgeted: 90_000,
    total_spent_budgeted: 68_000,
    total_remaining: 22_000,
    unbudgeted_spent: 2_500,
    uncategorised_spent: 700,
    income: 300_000,
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  m.budgetMonth.mockImplementation(async (month) => budget(month));
  m.listCategories.mockResolvedValue(TREE);
  m.parseMoney.mockImplementation(async (s: string) => Math.round(Number(s.replace(/[$,]/g, "")) * 100));
});

function renderPage() {
  const user = userEvent.setup();
  render(
    <ToastProvider>
      <BudgetPage initialMonth="2026-11" />
    </ToastProvider>,
  );
  return user;
}

const row = (name: string) => screen.getByRole("cell", { name }).closest("tr")!;

describe("BudgetPage", () => {
  it("shows the month, totals and grouped lines", async () => {
    renderPage();
    expect(await screen.findByRole("heading", { name: "November 2026" })).toBeInTheDocument();
    expect(await screen.findByText("$900.00")).toBeInTheDocument();
    expect(screen.getByText("$680.00")).toBeInTheDocument();
    expect(screen.getByText(/Spending without a budget \$25\.00 · Uncategorised \$7\.00 · Income \$3,000\.00/)).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Food" })).toBeInTheDocument();
    const g = row("Groceries");
    expect(within(g).getByRole("button", { name: "Edit budget for Groceries" })).toHaveTextContent("$800.00");
    expect(within(g).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "68");
    expect(within(row("Eating out")).getByRole("button", { name: "Edit budget for Eating out" })).toHaveTextContent("Set");
    expect(within(row("Eating out")).queryByRole("progressbar")).toBeNull();
  });

  it("flags overspending and shows carried amounts", async () => {
    renderPage();
    await screen.findByText("$900.00");
    const fuel = row("Fuel");
    expect(within(fuel).getByText("-$20.00")).toHaveClass("negative");
    expect(within(fuel).getByText("+$20.00 carried")).toBeInTheDocument();
    expect(within(fuel).getByRole("progressbar")).toHaveClass("over");
    expect(within(fuel).getByRole("checkbox", { name: "Roll over unspent Fuel" })).toBeChecked();
  });

  it("switches months", async () => {
    const user = renderPage();
    await screen.findByText("$900.00");
    await user.click(screen.getByRole("button", { name: "Previous month" }));
    expect(await screen.findByRole("heading", { name: "October 2026" })).toBeInTheDocument();
    expect(m.budgetMonth).toHaveBeenLastCalledWith("2026-10");
    await user.click(screen.getByRole("button", { name: "Next month" }));
    await user.click(screen.getByRole("button", { name: "Next month" }));
    expect(m.budgetMonth).toHaveBeenLastCalledWith("2026-12");
  });

  it("edits a budget inline: Enter saves once, from this month", async () => {
    const user = renderPage();
    await screen.findByText("$900.00");
    await user.click(screen.getByRole("button", { name: "Edit budget for Eating out" }));
    const input = screen.getByRole("textbox", { name: "Budget for Eating out" });
    await vi.waitFor(() => expect(input).toHaveFocus());
    await user.keyboard("150{Enter}");
    expect(m.setBudget).toHaveBeenCalledTimes(1);
    expect(m.setBudget).toHaveBeenCalledWith(11, "2026-11", 15_000);
    expect(screen.queryByRole("textbox", { name: "Budget for Eating out" })).toBeNull();
  });

  it("Escape cancels without saving; clearing the amount sets 0", async () => {
    const user = renderPage();
    await screen.findByText("$900.00");
    await user.click(screen.getByRole("button", { name: "Edit budget for Groceries" }));
    await user.clear(screen.getByRole("textbox", { name: "Budget for Groceries" }));
    await user.keyboard("{Escape}");
    expect(m.setBudget).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Edit budget for Groceries" }));
    await user.clear(screen.getByRole("textbox", { name: "Budget for Groceries" }));
    await user.keyboard("{Enter}");
    expect(m.setBudget).toHaveBeenCalledWith(10, "2026-11", 0);
  });

  it("bad amounts show an error and keep editing", async () => {
    m.parseMoney.mockRejectedValueOnce({ code: "invalid_money", message: "Enter an amount like 12.50." });
    const user = renderPage();
    await screen.findByText("$900.00");
    await user.click(screen.getByRole("button", { name: "Edit budget for Eating out" }));
    await user.type(screen.getByRole("textbox", { name: "Budget for Eating out" }), "abc{Enter}");
    expect(await screen.findByText("Enter an amount like 12.50.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Budget for Eating out" })).toHaveAttribute("aria-invalid", "true");
    expect(m.setBudget).not.toHaveBeenCalled();
  });

  it("undo appears only for a change made this month; rollover toggles", async () => {
    const user = renderPage();
    await screen.findByText("$900.00");
    expect(within(row("Groceries")).queryByRole("button", { name: /Undo/ })).toBeNull();
    await user.click(within(row("Fuel")).getByRole("button", { name: "Undo this month's change to Fuel" }));
    expect(m.clearBudget).toHaveBeenCalledWith(20, "2026-11");
    await user.click(within(row("Groceries")).getByRole("checkbox", { name: "Roll over unspent Groceries" }));
    expect(m.setBudgetRollover).toHaveBeenCalledWith(10, true);
  });
});
