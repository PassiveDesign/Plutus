import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components";
import { Account, api, ImportPreview, ImportPreviewRow, ImportStatus } from "../lib/api";
import { ImportPage } from "./ImportPage";

vi.mock("../lib/api", async (orig) => {
  const real = await orig<typeof import("../lib/api")>();
  return {
    ...real,
    api: {
      ...real.api,
      listAccounts: vi.fn(),
      previewBomImport: vi.fn(),
      commitImport: vi.fn(),
      listCategories: vi.fn(),
    },
  };
});
const m = vi.mocked(api);

function acct(id: number, name: string, institution: string | null = null): Account {
  return {
    id,
    name,
    kind: "offset",
    institution,
    bsb: null,
    account_number: null,
    opening_balance: 0,
    opening_date: "2026-11-01",
    offset_for: null,
    notes: null,
    archived_at: null,
    created_at: "x",
  };
}

function row(line: number, status: ImportStatus, payee: string | null, amount: number, balance: number | null = null): ImportPreviewRow {
  return {
    line,
    date: "2026-11-18",
    amount,
    description: `BANK TEXT ${line}`,
    payee,
    balance,
    import_key: `k${line}`,
    status,
    matched_transaction_id: status === "possible_match" ? 77 : null,
    category_id: null,
  };
}

function preview(over: Partial<ImportPreview> = {}): ImportPreview {
  return {
    account_id: 2,
    rows: [
      row(2, "new", "Corner Bakery", -650, 241_275),
      row(3, "duplicate", null, 250_000, 241_925),
      row(4, "possible_match", "Fresh Mart", -8_430, -8_075),
      row(5, "before_opening", null, -500, 355),
    ],
    order: "newest_first",
    balance_breaks: [],
    bank_balance: 241_275,
    current_balance: 241_925,
    balance_after: 241_275,
    ...over,
  };
}

const CSV = "Date,Description,Debit,Credit,Balance\r\n11/18/2026,X,6.50,,2412.75\r\n";

function renderPage(onView = vi.fn()) {
  const user = userEvent.setup();
  render(
    <ToastProvider>
      <ImportPage onViewTransactions={onView} />
    </ToastProvider>,
  );
  return { user, onView };
}

async function upload(user: ReturnType<typeof userEvent.setup>, text = CSV, name = "trans201126.csv") {
  await user.upload(screen.getByLabelText("Bank export file"), new File([text], name, { type: "text/csv" }));
}

beforeEach(() => {
  vi.resetAllMocks();
  m.listAccounts.mockResolvedValue([acct(1, "Bills Offset"), acct(2, "Everyday Offset", "Bank of Melbourne")]);
  m.previewBomImport.mockResolvedValue(preview());
  m.commitImport.mockResolvedValue({ imported: 2, skipped: 0 });
  m.listCategories.mockResolvedValue([
    {
      id: 1, name: "Food", kind: "expense", sort_order: 1, archived_at: null,
      categories: [{ id: 11, group_id: 1, name: "Takeaway & coffee", sort_order: 1, archived_at: null }],
    },
  ]);
});

describe("ImportPage", () => {
  it("defaults to the Bank of Melbourne account and previews the chosen file", async () => {
    const { user } = renderPage();
    expect(await screen.findByLabelText("Import into")).toHaveValue("2");
    await upload(user);
    expect(m.previewBomImport).toHaveBeenCalledWith(2, CSV);
    expect(await screen.findByText("trans201126.csv")).toBeInTheDocument();
    expect(screen.getByText(/1 new · 1 already imported · 1 possible match · 1 before opening date/)).toBeInTheDocument();
    expect(screen.getByText("Corner Bakery")).toBeInTheDocument();
    expect(screen.getByText("$2,412.75")).toBeInTheDocument();
  });

  it("ticks new rows only; duplicates can't be ticked; others can", async () => {
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    const box = (line: number) => screen.getByRole("checkbox", { name: new RegExp(`^Import line ${line}:`) });
    expect(await screen.findByRole("checkbox", { name: /^Import line 2:/ })).toBeChecked();
    expect(box(3)).not.toBeChecked();
    expect(box(3)).toBeDisabled();
    expect(box(4)).not.toBeChecked();
    expect(box(5)).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Import 1 transaction" })).toBeEnabled();

    await user.click(box(4));
    await user.click(screen.getByRole("button", { name: "Import 2 transactions" }));
    expect(m.commitImport).toHaveBeenCalledWith(2, [
      expect.objectContaining({ line: 2, import_key: "k2", payee: "Corner Bakery" }),
      expect.objectContaining({ line: 4, import_key: "k4" }),
    ]);
    const sent = m.commitImport.mock.calls[0][1][0];
    expect(sent).not.toHaveProperty("status");
    expect(sent).not.toHaveProperty("matched_transaction_id");
  });

  it("after importing, offers to view the account's transactions", async () => {
    const { user, onView } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    await user.click(await screen.findByRole("button", { name: "Import 1 transaction" }));
    expect(await screen.findByText("Imported 2 transactions into Everyday Offset.")).toBeInTheDocument();
    expect(screen.queryByRole("table")).toBeNull();
    await user.click(screen.getByRole("button", { name: "View in Transactions" }));
    expect(onView).toHaveBeenCalledWith(2);
  });

  it("nothing ticked disables Import", async () => {
    m.previewBomImport.mockResolvedValue(preview({ rows: [row(2, "duplicate", "Old", -100)] }));
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    expect(await screen.findByRole("button", { name: "Nothing to import" })).toBeDisabled();
  });

  it("warns when the projected balance differs from the bank's", async () => {
    m.previewBomImport.mockResolvedValue(preview({ balance_after: 100_000, balance_breaks: [7, 9] }));
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    const notes = await screen.findAllByRole("note");
    expect(notes[0]).toHaveTextContent("Plutus will show $1,000.00 for Everyday Offset, but the bank file says $2,412.75");
    expect(notes[1]).toHaveTextContent("near lines 7, 9");
  });

  it("no warning when balances agree", async () => {
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    await screen.findByText("Corner Bakery");
    expect(screen.queryByRole("note")).toBeNull();
  });

  it("file errors are shown and clear the preview", async () => {
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    await screen.findByText("Corner Bakery");
    m.previewBomImport.mockRejectedValueOnce({
      code: "invalid",
      field: "file",
      message: "This doesn't look like a Bank of Melbourne export.",
    });
    await upload(user, "junk", "other.csv");
    expect(await screen.findByRole("alert")).toHaveTextContent("This doesn't look like a Bank of Melbourne export.");
    expect(screen.queryByText("Corner Bakery")).toBeNull();
  });

  it("changing the account re-previews the same file", async () => {
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    await screen.findByText("Corner Bakery");
    await user.selectOptions(screen.getByLabelText("Import into"), "1");
    expect(m.previewBomImport).toHaveBeenLastCalledWith(1, CSV);
  });

  it("a failed import says nothing was saved and keeps the preview", async () => {
    m.commitImport.mockRejectedValueOnce({ code: "invalid", field: "file", message: "Line 5: Date must be between 1900 and 2100." });
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    await user.click(await screen.findByRole("button", { name: "Import 1 transaction" }));
    expect(await screen.findByText(/Import failed, nothing was saved: Line 5/)).toBeInTheDocument();
    expect(within(screen.getByRole("table")).getByText("Corner Bakery")).toBeInTheDocument();
  });

  it("shows the category a rule will set, and doesn't send it back", async () => {
    const p = preview();
    p.rows[0] = { ...p.rows[0], category_id: 11 };
    m.previewBomImport.mockResolvedValue(p);
    const { user } = renderPage();
    await screen.findByLabelText("Import into");
    await upload(user);
    const bakery = (await screen.findByText("Corner Bakery")).closest("tr")!;
    expect(within(bakery).getByTitle("Set by a rule: Food › Takeaway & coffee")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Import 1 transaction" }));
    expect(m.commitImport.mock.calls[0][1][0]).not.toHaveProperty("category_id");
  });

  it("asks for an account first when there are none", async () => {
    m.listAccounts.mockResolvedValue([]);
    renderPage();
    expect(await screen.findByText("Add an account first, then import its bank export.")).toBeInTheDocument();
  });
});
