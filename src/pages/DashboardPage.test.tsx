import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { Account, api, Dashboard } from "../lib/api";
import { DashboardPage } from "./DashboardPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      dashboard: vi.fn(),
      listAccounts: vi.fn(),
      accountBalances: vi.fn(),
      listCategories: vi.fn(),
    },
  };
});
const m = vi.mocked(api);

function acct(id: number, name: string): Account {
  return {
    id, name, kind: "offset", institution: null, bsb: null, account_number: null,
    opening_balance: 0, opening_date: "2026-01-01", offset_for: null, notes: null, archived_at: null, created_at: "x",
  };
}

function dash(over: Partial<Dashboard> = {}): Dashboard {
  return {
    month: "2026-11",
    net_worth: { assets: 1_250_000, debts: -48_000_000, net: -46_750_000 },
    totals: { income: 600_000, spending: 245_050, net: 354_950 },
    top_spending: [
      { category_id: 10, spent: 54_000 },
      { category_id: null, spent: 7_000 },
    ],
    budgeted: 90_000,
    budget_spent: 68_000,
    budget_remaining: 22_000,
    recent: [
      {
        id: 1, account_id: 1, date: "2026-11-18", amount: -650, payee: "Corner Bakery", description: "EFTPOS",
        memo: null, cleared: true, created_at: "x", updated_at: null, transfer_id: null, transfer_account_id: null, category_id: 12,
      },
    ],
    ...over,
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  m.dashboard.mockResolvedValue(dash());
  m.listAccounts.mockResolvedValue([acct(1, "Everyday Offset"), acct(2, "Home Loan")]);
  m.accountBalances.mockResolvedValue({ "1": 1_250_000, "2": -48_000_000 });
  m.listCategories.mockResolvedValue([
    {
      id: 2, name: "Food", kind: "expense", sort_order: 2, archived_at: null,
      categories: [{ id: 10, group_id: 2, name: "Groceries", sort_order: 1, archived_at: null }],
    },
  ]);
});

function renderPage() {
  const user = userEvent.setup();
  const onNavigate = vi.fn();
  const onOpenAccount = vi.fn();
  render(
    <ToastProvider>
      <DashboardPage onNavigate={onNavigate} onOpenAccount={onOpenAccount} />
    </ToastProvider>,
  );
  return { user, onNavigate, onOpenAccount };
}

describe("DashboardPage", () => {
  it("shows net worth, this month and budget", async () => {
    renderPage();
    const nw = await screen.findByRole("region", { name: "Net worth" });
    expect(within(nw).getByText("-$467,500.00")).toHaveClass("negative");
    expect(nw).toHaveTextContent("Assets $12,500.00 · Debts -$480,000.00");
    const budget = screen.getByRole("region", { name: "Budget" });
    expect(budget).toHaveTextContent("$220.00left of $900.00");
    expect(within(budget).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "76");
    expect(screen.getByText("In $6,000.00 · Out $2,450.50")).toBeInTheDocument();
  });

  it("lists top spending, accounts and recent transactions", async () => {
    const { user, onOpenAccount, onNavigate } = renderPage();
    const top = await screen.findByRole("region", { name: "Top spending this month" });
    const items = within(top).getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("Groceries$540.00");
    expect(items[1]).toHaveTextContent("Uncategorised$70.00");
    await user.click(screen.getByRole("button", { name: /Home Loan/ }));
    expect(onOpenAccount).toHaveBeenCalledWith(2);
    const recent = screen.getByRole("region", { name: "Recent transactions" });
    expect(recent).toHaveTextContent("18 Nov 2026");
    expect(recent).toHaveTextContent("Corner Bakery");
    expect(recent).toHaveTextContent("Everyday Offset");
    await user.click(within(recent).getByRole("button", { name: "View all" }));
    expect(onNavigate).toHaveBeenCalledWith("transactions");
  });

  it("without budgets or spending, offers next steps", async () => {
    m.dashboard.mockResolvedValue(dash({ budgeted: 0, budget_spent: 0, budget_remaining: 0, top_spending: [], recent: [] }));
    const { user, onNavigate } = renderPage();
    await user.click(await screen.findByRole("button", { name: "Set budgets" }));
    expect(onNavigate).toHaveBeenCalledWith("budgets");
    expect(screen.getByText("No spending yet this month.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Import a bank export" }));
    expect(onNavigate).toHaveBeenCalledWith("import");
  });

  it("with no accounts, welcomes and points to Accounts", async () => {
    m.listAccounts.mockResolvedValue([]);
    const { user, onNavigate } = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add an account" }));
    expect(onNavigate).toHaveBeenCalledWith("accounts");
  });
});
