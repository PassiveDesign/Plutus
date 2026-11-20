import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { Account, api, Transaction, TransactionInput, TransactionQuery } from "../lib/api";
import { TransactionsPage } from "./TransactionsPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      listAccounts: vi.fn(),
      listTransactions: vi.fn(),
      countTransactions: vi.fn(),
      addTransaction: vi.fn(),
      updateTransaction: vi.fn(),
      deleteTransaction: vi.fn(),
      parseMoney: vi.fn(),
    },
  };
});
const m = vi.mocked(api);

function acct(id: number, name: string, over: Partial<Account> = {}): Account {
  return {
    id,
    name,
    kind: "offset",
    institution: null,
    bsb: null,
    account_number: null,
    opening_balance: 0,
    opening_date: "2026-11-01",
    offset_for: null,
    notes: null,
    archived_at: null,
    created_at: "x",
    ...over,
  };
}

let accounts: Account[];
let txs: Transaction[];
let nextId: number;

function tx(account_id: number, date: string, amount: number, payee: string | null, over: Partial<Transaction> = {}): Transaction {
  return {
    id: nextId++,
    account_id,
    date,
    amount,
    payee,
    description: null,
    memo: null,
    cleared: false,
    created_at: "x",
    updated_at: null,
    ...over,
  };
}

/** In-memory backend mirroring plutus_core::transactions::list/count. */
function matching(q: TransactionQuery) {
  return txs
    .filter((t) => q.account_id == null || t.account_id === q.account_id)
    .filter((t) => !q.from || t.date >= q.from)
    .filter((t) => !q.to || t.date <= q.to)
    .sort((a, b) => (a.date === b.date ? b.id - a.id : a.date < b.date ? 1 : -1));
}

beforeEach(() => {
  vi.resetAllMocks();
  nextId = 1;
  accounts = [acct(1, "Everyday Offset"), acct(2, "Bills Offset"), acct(3, "Old Saver", { archived_at: "y" })];
  txs = [];
  m.listAccounts.mockImplementation(async () => accounts);
  m.listTransactions.mockImplementation(async (q: TransactionQuery = {}) => {
    const off = q.offset ?? 0;
    return matching(q).slice(off, off + (q.limit ?? 200));
  });
  m.countTransactions.mockImplementation(async (q: TransactionQuery = {}) => matching(q).length);
  m.parseMoney.mockImplementation(async (text: string) => {
    const t = text.replace(/[$,\s]/g, "");
    if (!/^-?\d+(\.\d{1,2})?$/.test(t)) throw { code: "invalid_amount", message: "Enter an amount like 1,234.56." };
    return Math.round(Number(t) * 100);
  });
  m.addTransaction.mockImplementation(async (input: TransactionInput) => {
    const t = { ...tx(input.account_id, input.date, input.amount, input.payee), ...input };
    txs.push(t);
    return t;
  });
  m.updateTransaction.mockImplementation(async (id: number, input: TransactionInput) => {
    const i = txs.findIndex((t) => t.id === id);
    txs[i] = { ...txs[i], ...input };
    return txs[i];
  });
  m.deleteTransaction.mockImplementation(async (id: number) => {
    txs = txs.filter((t) => t.id !== id);
  });
});

function renderPage() {
  render(
    <ToastProvider>
      <TransactionsPage />
    </ToastProvider>,
  );
  return userEvent.setup();
}

const bodyRows = () => screen.getAllByRole("row").slice(1);

describe("TransactionsPage", () => {
  it("with no active accounts, explains and disables adding", async () => {
    accounts = [acct(3, "Old Saver", { archived_at: "y" })];
    renderPage();
    expect(await screen.findByText(/Add an account first/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add transaction" })).toBeDisabled();
  });

  it("lists newest first with dates, payee/description, account and signed amounts", async () => {
    txs = [
      tx(1, "2026-11-10", -8_745, "Woolworths", { description: "WOOLWORTHS 3021 MELB", memo: "groceries" }),
      tx(2, "2026-11-15", 350_000, null, { description: "SALARY ACME PTY LTD" }),
    ];
    renderPage();
    await screen.findByText("SALARY ACME PTY LTD");
    const [first, second] = bodyRows();
    expect(first).toHaveTextContent("15 Nov 2026");
    expect(first).toHaveTextContent("Bills Offset");
    expect(within(first).getByText("$3,500.00")).not.toHaveClass("negative");
    expect(second).toHaveTextContent("WOOLWORTHS 3021 MELB · groceries");
    expect(within(second).getByText("-$87.45")).toHaveClass("negative");
    expect(screen.getByText("1–2 of 2")).toBeInTheDocument();
  });

  it("filtering by account hides the account column and queries by account", async () => {
    txs = [tx(1, "2026-11-10", -100, "A"), tx(2, "2026-11-11", -200, "B")];
    const user = renderPage();
    await screen.findByText("B");
    await user.selectOptions(screen.getByLabelText("Account"), "1");
    await vi.waitFor(() => expect(bodyRows()).toHaveLength(1));
    expect(m.listTransactions).toHaveBeenLastCalledWith(expect.objectContaining({ account_id: 1, offset: 0 }));
    expect(screen.queryByRole("columnheader", { name: "Account" })).toBeNull();
  });

  it("date filters are inclusive and say when nothing matches", async () => {
    txs = [tx(1, "2026-11-10", -100, "Ten"), tx(1, "2026-11-12", -100, "Twelve")];
    const user = renderPage();
    await screen.findByText("Twelve");
    await user.type(screen.getByLabelText("From"), "2026-11-12");
    await vi.waitFor(() => expect(bodyRows()).toHaveLength(1));
    expect(screen.getByText("Twelve")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("From"));
    await user.type(screen.getByLabelText("From"), "2026-11-13");
    expect(await screen.findByText("No transactions match these filters.")).toBeInTheDocument();
  });

  it("pages through results 100 at a time", async () => {
    for (let i = 0; i < 150; i++) txs.push(tx(1, "2026-11-10", -1, `Tx ${i}`));
    const user = renderPage();
    expect(await screen.findByText("1–100 of 150")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect(await screen.findByText("101–150 of 150")).toBeInTheDocument();
    expect(bodyRows()).toHaveLength(50);
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    // Changing a filter returns to page 1.
    await user.selectOptions(screen.getByLabelText("Account"), "1");
    expect(await screen.findByText("1–100 of 150")).toBeInTheDocument();
  });

  it("adds money out as a negative amount, defaulting to the filtered account", async () => {
    const user = renderPage();
    await screen.findByText("No transactions yet.");
    await user.selectOptions(screen.getByLabelText("Account"), "2");
    await user.click(screen.getByRole("button", { name: "Add transaction" }));
    const dialog = screen.getByRole("dialog", { name: "Add transaction" });
    expect(within(dialog).getByLabelText("Account")).toHaveValue("2");
    // Archived accounts are not offered.
    expect(within(dialog).queryByRole("option", { name: /Old Saver/ })).toBeNull();
    expect(within(dialog).getByLabelText("Money out")).toBeChecked();
    await user.type(within(dialog).getByLabelText("Amount"), "87.45");
    await user.type(within(dialog).getByLabelText("Payee"), "Woolworths");
    await user.click(within(dialog).getByRole("button", { name: "Add transaction" }));
    expect(m.addTransaction).toHaveBeenCalledWith(
      expect.objectContaining({ account_id: 2, amount: -8_745, payee: "Woolworths", description: null, cleared: false }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("Transaction added.");
    expect(await screen.findByText("Woolworths")).toBeInTheDocument();
  });

  it("money in is positive; a typed minus sign doesn't flip it back", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add transaction" }));
    const dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByLabelText("Money in"));
    await user.type(within(dialog).getByLabelText("Amount"), "-3,500");
    await user.type(within(dialog).getByLabelText("Payee"), "Salary");
    await user.click(within(dialog).getByRole("button", { name: "Add transaction" }));
    expect(m.addTransaction).toHaveBeenCalledWith(expect.objectContaining({ account_id: 1, amount: 350_000 }));
  });

  it("amount and backend field errors show on the right inputs", async () => {
    m.addTransaction.mockRejectedValueOnce({ code: "invalid", field: "payee", message: "Enter a payee or description." });
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add transaction" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Amount"), "1.234");
    await user.click(within(dialog).getByRole("button", { name: "Add transaction" }));
    expect(within(dialog).getByLabelText("Amount")).toHaveAccessibleDescription("Enter an amount like 1,234.56.");
    expect(m.addTransaction).not.toHaveBeenCalled();

    await user.clear(within(dialog).getByLabelText("Amount"));
    await user.type(within(dialog).getByLabelText("Amount"), "5");
    await user.click(within(dialog).getByRole("button", { name: "Add transaction" }));
    expect(await within(dialog).findByText("Enter a payee or description.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Payee")).toHaveAttribute("aria-invalid", "true");
  });

  it("the cleared checkbox toggles without opening the editor", async () => {
    txs = [tx(1, "2026-11-10", -500, "Coffee")];
    const user = renderPage();
    await user.click(await screen.findByRole("checkbox", { name: "Cleared: Coffee" }));
    expect(m.updateTransaction).toHaveBeenCalledWith(
      txs[0].id,
      expect.objectContaining({ cleared: true, amount: -500, payee: "Coffee" }),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
    await vi.waitFor(() => expect(screen.getByRole("checkbox", { name: "Cleared: Coffee" })).toBeChecked());
  });

  it("edits keep direction and amount; delete needs a second click", async () => {
    txs = [tx(1, "2026-11-10", 120_000, "Refund")];
    const user = renderPage();
    await user.click(await screen.findByText("Refund"));
    let dialog = screen.getByRole("dialog", { name: "Edit transaction" });
    expect(within(dialog).getByLabelText("Money in")).toBeChecked();
    expect(within(dialog).getByLabelText("Amount")).toHaveValue("1200.00");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(m.updateTransaction).toHaveBeenLastCalledWith(1, expect.objectContaining({ amount: 120_000 }));

    await user.click(await screen.findByText("Refund"));
    dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect(m.deleteTransaction).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Really delete?" }));
    expect(m.deleteTransaction).toHaveBeenCalledWith(1);
    expect(await screen.findByText("No transactions yet.")).toBeInTheDocument();
  });

  it("editing a transaction in an archived account keeps that account selectable", async () => {
    txs = [tx(3, "2026-11-10", -100, "Old fee")];
    const user = renderPage();
    await user.click(await screen.findByText("Old fee"));
    const picker = within(screen.getByRole("dialog")).getByLabelText("Account");
    expect(picker).toHaveValue("3");
    expect(within(picker).getByRole("option", { name: "Old Saver (archived)" })).toBeInTheDocument();
  });

  it("load failure shows a retry", async () => {
    m.countTransactions.mockRejectedValueOnce({ code: "locked", message: "the vault is locked" });
    const user = renderPage();
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't load transactions: The vault is locked.");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("No transactions yet.")).toBeInTheDocument();
  });
});
