import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import {
  Account,
  api,
  LedgerRow,
  Transaction,
  TransactionInput,
  TransactionQuery,
  TransferInput,
} from "../lib/api";
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
      createTransfer: vi.fn(),
      updateTransfer: vi.fn(),
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
    transfer_id: null,
    transfer_account_id: null,
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

/** Running balance per transaction id: opening + rows on/after the opening
 *  date, in date then id order (mirrors list_with_running_balance). */
function runningFor(accountId: number): Map<number, number> {
  const a = accounts.find((x) => x.id === accountId)!;
  const out = new Map<number, number>();
  let bal = a.opening_balance;
  for (const t of txs
    .filter((x) => x.account_id === accountId)
    .sort((x, y) => (x.date === y.date ? x.id - y.id : x.date < y.date ? -1 : 1))) {
    if (t.date < a.opening_date) continue;
    bal += t.amount;
    out.set(t.id, bal);
  }
  return out;
}

beforeEach(() => {
  vi.resetAllMocks();
  nextId = 1;
  accounts = [acct(1, "Everyday Offset"), acct(2, "Bills Offset"), acct(3, "Old Saver", { archived_at: "y" })];
  txs = [];
  m.listAccounts.mockImplementation(async () => accounts);
  m.listTransactions.mockImplementation(async (q: TransactionQuery = {}) => {
    const off = q.offset ?? 0;
    const running = q.account_id == null ? new Map<number, number>() : runningFor(q.account_id);
    return matching(q)
      .slice(off, off + (q.limit ?? 200))
      .map((t): LedgerRow => ({ ...t, running_balance: running.get(t.id) ?? null }));
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
    const group = txs.find((t) => t.id === id)?.transfer_id;
    txs = txs.filter((t) => t.id !== id && (group == null || t.transfer_id !== group));
  });
  m.createTransfer.mockImplementation(async (input: TransferInput) => {
    const name = (id: number) => accounts.find((a) => a.id === id)!.name;
    const out = tx(input.from_account, input.date, -input.amount, `Transfer to ${name(input.to_account)}`, {
      memo: input.memo,
      transfer_account_id: input.to_account,
    });
    const inc = tx(input.to_account, input.date, input.amount, `Transfer from ${name(input.from_account)}`, {
      memo: input.memo,
      transfer_account_id: input.from_account,
    });
    out.transfer_id = inc.transfer_id = out.id;
    txs.push(out, inc);
    return { id: out.id, from: out, to: inc };
  });
  m.updateTransfer.mockImplementation(async (id: number) => {
    const [from, to] = txs.filter((t) => t.transfer_id === id).sort((a, b) => a.amount - b.amount);
    return { id, from, to };
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

  it("shows a running balance only when filtered to one account", async () => {
    accounts[0] = { ...accounts[0], opening_balance: 100_000, opening_date: "2026-11-01" };
    txs = [
      tx(1, "2026-10-20", -5_000, "Before opening"),
      tx(1, "2026-11-05", -2_500, "Groceries"),
      tx(1, "2026-11-10", 350_000, "Salary"),
      tx(2, "2026-11-06", -1_000, "Other account"),
    ];
    const user = renderPage();
    await screen.findByText("Salary");
    expect(screen.queryByRole("columnheader", { name: "Balance" })).toBeNull();

    await user.selectOptions(screen.getByLabelText("Account"), "1");
    expect(await screen.findByRole("columnheader", { name: "Balance" })).toBeInTheDocument();
    const [salary, groceries, before] = bodyRows();
    expect(salary).toHaveTextContent("$4,475.00");
    expect(groceries).toHaveTextContent("$975.00");
    // Pre-opening rows are already in the opening balance.
    expect(within(before).getByTitle("Before the account's opening date")).toHaveTextContent("—");
  });

  it("negative running balances are flagged", async () => {
    accounts[0] = { ...accounts[0], kind: "credit_card", opening_balance: -10_000 };
    txs = [tx(1, "2026-11-05", -2_500, "Fuel")];
    const user = renderPage();
    await screen.findByText("Fuel");
    await user.selectOptions(screen.getByLabelText("Account"), "1");
    const cell = await screen.findByText("-$125.00", { selector: "span" });
    expect(cell).toHaveClass("money", "negative");
  });

  it("adds a transfer from the filtered account; payee fields are hidden", async () => {
    const user = renderPage();
    await screen.findByText("No transactions yet.");
    await user.selectOptions(screen.getByLabelText("Account"), "1");
    await user.click(screen.getByRole("button", { name: "Add transaction" }));
    const dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByLabelText("Transfer"));
    expect(within(dialog).getByRole("heading", { name: "Add transfer" })).toBeInTheDocument();
    expect(within(dialog).queryByLabelText("Payee")).toBeNull();
    expect(within(dialog).queryByLabelText(/Cleared/)).toBeNull();
    expect(within(dialog).getByLabelText("From account")).toHaveValue("1");
    expect(within(dialog).getByLabelText("To account")).toHaveValue("2"); // first other account
    await user.type(within(dialog).getByLabelText("Amount"), "250");
    await user.type(within(dialog).getByLabelText("Memo"), "Power bill money");
    await user.click(within(dialog).getByRole("button", { name: "Add transfer" }));
    expect(m.createTransfer).toHaveBeenCalledWith({
      from_account: 1,
      to_account: 2,
      date: expect.any(String),
      amount: 25_000,
      memo: "Power bill money",
    });
    expect(m.addTransaction).not.toHaveBeenCalled();
    expect(await screen.findByRole("status")).toHaveTextContent("Transfer added.");
    expect(await screen.findByText("Transfer to Bills Offset")).toBeInTheDocument();
  });

  it("transfer errors land on the matching picker", async () => {
    m.createTransfer.mockRejectedValueOnce({
      code: "invalid",
      field: "to_account",
      message: "Choose a different account to transfer to.",
    });
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add transaction" }));
    const dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByLabelText("Transfer"));
    await user.selectOptions(within(dialog).getByLabelText("To account"), "1");
    await user.type(within(dialog).getByLabelText("Amount"), "5");
    await user.click(within(dialog).getByRole("button", { name: "Add transfer" }));
    expect(within(dialog).getByLabelText("To account")).toHaveAccessibleDescription(
      "Choose a different account to transfer to.",
    );

    m.createTransfer.mockRejectedValueOnce({ code: "invalid", field: "from_account", message: "Old Saver is archived." });
    await user.selectOptions(within(dialog).getByLabelText("To account"), "2");
    await user.click(within(dialog).getByRole("button", { name: "Add transfer" }));
    expect(await within(dialog).findByText("Old Saver is archived.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("From account")).toHaveAttribute("aria-invalid", "true");
  });

  it("editing either leg of a transfer edits the transfer, from the outgoing side", async () => {
    const out = tx(1, "2026-11-15", -25_000, "Transfer to Bills Offset", { transfer_account_id: 2 });
    const inc = tx(2, "2026-11-15", 25_000, "Transfer from Everyday Offset", { transfer_account_id: 1 });
    out.transfer_id = inc.transfer_id = out.id;
    txs = [out, inc];
    const user = renderPage();
    await user.click(await screen.findByText("Transfer from Everyday Offset")); // incoming leg
    const dialog = screen.getByRole("dialog", { name: "Edit transfer" });
    expect(within(dialog).queryByRole("radio")).toBeNull(); // type is fixed
    expect(within(dialog).getByLabelText("From account")).toHaveValue("1");
    expect(within(dialog).getByLabelText("To account")).toHaveValue("2");
    expect(within(dialog).getByLabelText("Amount")).toHaveValue("250.00");
    await user.clear(within(dialog).getByLabelText("Amount"));
    await user.type(within(dialog).getByLabelText("Amount"), "300");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(m.updateTransfer).toHaveBeenCalledWith(out.id, expect.objectContaining({ from_account: 1, to_account: 2, amount: 30_000 }));
    expect(m.updateTransaction).not.toHaveBeenCalled();
  });

  it("deleting a transfer says both sides go", async () => {
    const out = tx(1, "2026-11-15", -100, "Transfer to Bills Offset", { transfer_account_id: 2 });
    const inc = tx(2, "2026-11-15", 100, "Transfer from Everyday Offset", { transfer_account_id: 1 });
    out.transfer_id = inc.transfer_id = out.id;
    txs = [out, inc];
    const user = renderPage();
    await user.click(await screen.findByText("Transfer to Bills Offset"));
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Really delete both sides?" }));
    expect(m.deleteTransaction).toHaveBeenCalledWith(out.id);
    expect(await screen.findByText("No transactions yet.")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Transfer deleted.");
  });

  it("an existing ordinary transaction can't be turned into a transfer", async () => {
    txs = [tx(1, "2026-11-10", -500, "Coffee")];
    const user = renderPage();
    await user.click(await screen.findByText("Coffee"));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByLabelText("Money out")).toBeChecked();
    expect(within(dialog).queryByLabelText("Transfer")).toBeNull();
  });

  it("can start filtered to an account (arriving from Import)", async () => {
    txs = [tx(1, "2026-11-05", -100, "In everyday"), tx(2, "2026-11-05", -200, "In bills")];
    render(
      <ToastProvider>
        <TransactionsPage initialAccountId={1} />
      </ToastProvider>,
    );
    expect(await screen.findByText("In everyday")).toBeInTheDocument();
    expect(screen.queryByText("In bills")).toBeNull();
    expect(screen.getByLabelText("Account")).toHaveValue("1");
  });

  it("load failure shows a retry", async () => {
    m.countTransactions.mockRejectedValueOnce({ code: "locked", message: "the vault is locked" });
    const user = renderPage();
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't load transactions: The vault is locked.");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("No transactions yet.")).toBeInTheDocument();
  });
});
