import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { Account, AccountInput, api } from "../lib/api";
import { AccountsPage } from "./AccountsPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      listAccounts: vi.fn(),
      createAccount: vi.fn(),
      updateAccount: vi.fn(),
      setAccountArchived: vi.fn(),
      parseMoney: vi.fn(),
    },
  };
});
const m = vi.mocked(api);

function acct(id: number, over: Partial<Account> = {}): Account {
  return {
    id,
    name: `Account ${id}`,
    kind: "offset",
    institution: null,
    bsb: null,
    account_number: null,
    opening_balance: 0,
    opening_date: "2026-11-01",
    offset_for: null,
    notes: null,
    archived_at: null,
    created_at: "2026-11-01T00:00:00Z",
    ...over,
  };
}

/** Tiny stand-in for Rust's Money::parse, enough for these tests. */
async function fakeParse(text: string): Promise<number> {
  const t = text.replace(/[$,\s]/g, "");
  if (!/^-?\d+(\.\d{1,2})?$/.test(t)) {
    throw { code: "invalid_amount", message: "Enter an amount like 1,234.56." };
  }
  return Math.round(Number(t) * 100);
}

let store: Account[];

beforeEach(() => {
  vi.resetAllMocks();
  store = [];
  m.listAccounts.mockImplementation(async () => store.map((a) => ({ ...a })));
  m.parseMoney.mockImplementation(fakeParse);
  m.createAccount.mockImplementation(async (input: AccountInput) => {
    const a = acct(store.length + 1, { ...input });
    store.push(a);
    return a;
  });
  m.updateAccount.mockImplementation(async (id: number, input: AccountInput) => {
    const i = store.findIndex((a) => a.id === id);
    store[i] = { ...store[i], ...input };
    return store[i];
  });
  m.setAccountArchived.mockImplementation(async (id: number, archived: boolean) => {
    const a = store.find((x) => x.id === id)!;
    a.archived_at = archived ? "2026-11-20T00:00:00Z" : null;
    return a;
  });
});

function renderPage() {
  render(
    <ToastProvider>
      <AccountsPage />
    </ToastProvider>,
  );
  return userEvent.setup();
}

describe("AccountsPage", () => {
  it("empty state invites adding the first account", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add your first account" }));
    expect(screen.getByRole("dialog", { name: "Add account" })).toBeInTheDocument();
    expect(m.listAccounts).toHaveBeenCalledWith(true);
  });

  it("groups accounts with totals, formats BSB and shows offset links", async () => {
    store = [
      acct(1, { name: "Home Loan", kind: "home_loan", opening_balance: -45_000_000 }),
      acct(2, { name: "Everyday Offset", bsb: "193879", account_number: "45678901", opening_balance: 1_234_567, offset_for: 1, institution: "Bank of Melbourne" }),
      acct(3, { name: "Bills Offset", opening_balance: 250_000, offset_for: 1 }),
    ];
    renderPage();
    const banking = await screen.findByRole("region", { name: /Banking/ });
    expect(within(banking).getByRole("heading")).toHaveTextContent("Banking$14,845.67");
    expect(within(banking).getByText("193-879 · 45678901")).toBeInTheDocument();
    expect(within(banking).getByText("Bank of Melbourne · Offsets Home Loan")).toBeInTheDocument();
    // Loan shows its offsets and what interest is charged on.
    expect(screen.getByText("2 offsets · Net owed $435,154.33")).toBeInTheDocument();
    const credit = screen.getByRole("region", { name: /Credit & loans/ });
    expect(within(credit).getByText("-$450,000.00", { selector: "td" })).toHaveClass("negative");
    expect(screen.getByText("-$435,154.33")).toHaveClass("negative"); // net total
  });

  it("adds a home loan: amount owed entered positive, stored negative", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add your first account" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "Home Loan");
    await user.selectOptions(within(dialog).getByLabelText("Type"), "home_loan");
    const owed = within(dialog).getByLabelText("Opening amount owed");
    await user.clear(owed);
    await user.type(owed, "$450,000");
    await user.type(within(dialog).getByLabelText("BSB"), "193-879");
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));

    expect(m.parseMoney).toHaveBeenCalledWith("$450,000");
    expect(m.createAccount).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "Home Loan",
        kind: "home_loan",
        opening_balance: -45_000_000,
        bsb: "193-879",
        institution: null,
        offset_for: null,
      }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("Home Loan added.");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(await screen.findByText("Home Loan")).toBeInTheDocument();
  });

  it("bad amounts are caught by the parser and shown on the field", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add your first account" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "Offset");
    const bal = within(dialog).getByLabelText("Opening balance");
    await user.clear(bal);
    await user.type(bal, "12.345");
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));
    expect(bal).toHaveAccessibleDescription("Enter an amount like 1,234.56.");
    expect(m.createAccount).not.toHaveBeenCalled();
  });

  it("backend field errors land on the right input", async () => {
    m.createAccount.mockRejectedValue({
      code: "conflict",
      field: "name",
      message: 'An active account named "Offset" already exists.',
    });
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add your first account" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "Offset");
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));
    expect(await within(dialog).findByText(/already exists/)).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Name")).toHaveAttribute("aria-invalid", "true");
    expect(within(dialog).getByRole("button", { name: "Add account" })).toBeEnabled();
  });

  it("errors without a matching field go to a toast", async () => {
    m.createAccount.mockRejectedValue({ code: "io", message: "i/o error: disk full" });
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add your first account" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "Offset");
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("I/o error: disk full.");
  });

  it("editing keeps an offset link; changing type away from offset drops it", async () => {
    store = [
      acct(1, { name: "Home Loan", kind: "home_loan", opening_balance: -100 }),
      acct(2, { name: "Everyday Offset", offset_for: 1, opening_balance: 500 }),
    ];
    const user = renderPage();
    await user.click(await screen.findByText("Everyday Offset"));
    let dialog = screen.getByRole("dialog", { name: "Edit Everyday Offset" });
    expect(within(dialog).getByLabelText("Opening balance")).toHaveValue("5.00");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(m.updateAccount).toHaveBeenLastCalledWith(2, expect.objectContaining({ offset_for: 1, opening_balance: 500 }));

    await user.click(await screen.findByText("Everyday Offset"));
    dialog = screen.getByRole("dialog");
    await user.selectOptions(within(dialog).getByLabelText("Type"), "savings");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(m.updateAccount).toHaveBeenLastCalledWith(2, expect.objectContaining({ kind: "savings", offset_for: null }));
  });

  it("liability balances round-trip through the edit form as positive amounts owed", async () => {
    store = [acct(1, { name: "Card", kind: "credit_card", opening_balance: -4250 })];
    const user = renderPage();
    await user.click(await screen.findByText("Card"));
    expect(screen.getByLabelText("Opening amount owed")).toHaveValue("42.50");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(m.updateAccount).toHaveBeenLastCalledWith(1, expect.objectContaining({ opening_balance: -4250 }));
  });

  it("archive hides the account; show archived reveals and restore brings it back", async () => {
    store = [acct(1, { name: "Old Saver", kind: "savings" }), acct(2, { name: "Everyday" })];
    const user = renderPage();
    await user.click(await screen.findByText("Old Saver"));
    await user.click(screen.getByRole("button", { name: "Archive" }));
    expect(m.setAccountArchived).toHaveBeenCalledWith(1, true);
    expect(await screen.findByRole("status")).toHaveTextContent("Old Saver archived.");
    await vi.waitFor(() => expect(screen.queryByText("Old Saver")).toBeNull());

    await user.click(screen.getByLabelText("Show archived (1)"));
    const row = (await screen.findByText("Old Saver")).closest("tr")!;
    expect(row).toHaveClass("archived");
    expect(within(row).getByText("Archived")).toBeInTheDocument();
    await user.click(row);
    await user.click(screen.getByRole("button", { name: "Restore" }));
    expect(m.setAccountArchived).toHaveBeenLastCalledWith(1, false);
  });

  it("offset picker lists active loans (plus a current archived link) and saves the choice", async () => {
    store = [
      acct(1, { name: "Home Loan", kind: "home_loan", opening_balance: -100 }),
      acct(2, { name: "Old Loan", kind: "loan", archived_at: "2026-11-20T00:00:00Z" }),
      acct(3, { name: "Card", kind: "credit_card" }),
      acct(4, { name: "Linked to old", offset_for: 2 }),
    ];
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add account" }));
    let dialog = screen.getByRole("dialog");
    const picker = within(dialog).getByLabelText("Offsets loan");
    expect([...picker.querySelectorAll("option")].map((o) => o.textContent)).toEqual([
      "Not linked",
      "Home Loan",
    ]);
    await user.type(within(dialog).getByLabelText("Name"), "Everyday Offset");
    await user.selectOptions(picker, "1");
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));
    expect(m.createAccount).toHaveBeenLastCalledWith(expect.objectContaining({ offset_for: 1 }));
    expect(await screen.findByText(/1 offset · Net owed/)).toBeInTheDocument();

    // Existing link to an archived loan stays selectable and selected.
    await user.click(screen.getByText("Linked to old"));
    dialog = screen.getByRole("dialog");
    const p2 = within(dialog).getByLabelText("Offsets loan") as HTMLSelectElement;
    expect(p2.value).toBe("2");
    expect(within(p2).getByText("Old Loan (archived)")).toBeInTheDocument();
    await user.selectOptions(p2, "");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(m.updateAccount).toHaveBeenLastCalledWith(4, expect.objectContaining({ offset_for: null }));
  });

  it("picker only shows for offset accounts and hints when there are no loans", async () => {
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add your first account" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByLabelText("Offsets loan")).toHaveAccessibleDescription(
      "Add your home loan as an account to link this offset to it.",
    );
    await user.selectOptions(within(dialog).getByLabelText("Type"), "savings");
    expect(within(dialog).queryByLabelText("Offsets loan")).toBeNull();
  });

  it("offset_for validation errors show on the picker", async () => {
    store = [acct(1, { name: "Home Loan", kind: "home_loan" })];
    m.createAccount.mockRejectedValue({ code: "invalid", field: "offset_for", message: "The linked loan is archived." });
    const user = renderPage();
    await user.click(await screen.findByRole("button", { name: "Add account" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "Offset");
    await user.selectOptions(within(dialog).getByLabelText("Offsets loan"), "1");
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));
    expect(await within(dialog).findByText("The linked loan is archived.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Offsets loan")).toHaveAttribute("aria-invalid", "true");
  });

  it("load failure shows a retry", async () => {
    m.listAccounts.mockRejectedValueOnce({ code: "locked", message: "the vault is locked" });
    const user = renderPage();
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't load accounts: The vault is locked.");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: "Add your first account" })).toBeInTheDocument();
  });
});
