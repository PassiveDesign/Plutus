import { ChangeEvent, useEffect, useMemo, useRef, useState } from "react";
import { FileUp } from "lucide-react";
import { Button, DataTable, SelectField, useToast } from "../components";
import type { Column } from "../components";
import {
  Account,
  api,
  errorMessage,
  CategoryGroup,
  ImportPreview,
  ImportPreviewRow,
  ImportRow,
  ImportStatus,
  splitError,
} from "../lib/api";
import { formatDate } from "../lib/dates";
import { categoryIndex } from "./CategorySelect";
import { formatCents } from "../lib/money";

const STATUS_LABELS: Record<ImportStatus, string> = {
  new: "New",
  duplicate: "Already imported",
  possible_match: "Possible match",
  before_opening: "Before opening date",
};

const STATUS_HINTS: Record<ImportStatus, string> = {
  new: "Not in Plutus yet.",
  duplicate: "Imported from an earlier file. Always skipped.",
  possible_match: "Same date and amount as a transaction you entered by hand. Tick to import anyway.",
  before_opening:
    "Dated before the account's opening date, so already counted in the opening balance. Tick to import anyway.",
};

type Props = {
  /** Show the imported rows on the Transactions page, filtered to the account. */
  onViewTransactions: (accountId: number) => void;
};

async function readText(file: File): Promise<string> {
  if (typeof file.text === "function") return file.text();
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result ?? ""));
    r.onerror = () => reject(r.error);
    r.readAsText(file);
  });
}

/** Strip the preview-only fields before sending rows back. */
function toImportRow({ status: _s, matched_transaction_id: _m, category_id: _c, ...row }: ImportPreviewRow): ImportRow {
  return row;
}

export function ImportPage({ onViewTransactions }: Props) {
  const toast = useToast();
  const fileInput = useRef<HTMLInputElement>(null);
  const [accounts, setAccounts] = useState<Account[] | null>(null);
  const [categories, setCategories] = useState<CategoryGroup[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [accountId, setAccountId] = useState<number | null>(null);
  const [file, setFile] = useState<{ name: string; text: string } | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [ticked, setTicked] = useState<Set<number>>(new Set());
  const [fileError, setFileError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<{ accountId: number; imported: number; skipped: number } | null>(null);

  useEffect(() => {
    api
      .listCategories(true)
      .then(setCategories)
      .catch(() => setCategories([]));
    api
      .listAccounts(false)
      .then((accts) => {
        setAccounts(accts);
        const bom = accts.find((a) => /melbourne/i.test(a.institution ?? ""));
        setAccountId((bom ?? accts[0])?.id ?? null);
      })
      .catch((e) => setLoadError(errorMessage(e)));
  }, []);

  async function runPreview(acct: number, f: { name: string; text: string }) {
    setBusy(true);
    setFileError(null);
    try {
      const p = await api.previewBomImport(acct, f.text);
      setPreview(p);
      setTicked(new Set(p.rows.filter((r) => r.status === "new").map((r) => r.line)));
    } catch (e) {
      const { field, general } = splitError(e);
      setPreview(null);
      setFileError(field.file ?? field.account_id ?? general ?? "Couldn't read the file.");
    } finally {
      setBusy(false);
    }
  }

  async function chooseFile(e: ChangeEvent<HTMLInputElement>) {
    const picked = e.currentTarget.files?.[0];
    e.currentTarget.value = ""; // allow re-picking the same file
    if (!picked || accountId === null) return;
    setDone(null);
    let text: string;
    try {
      text = await readText(picked);
    } catch (err) {
      setFileError(`Couldn't open ${picked.name}: ${errorMessage(err)}`);
      return;
    }
    const f = { name: picked.name, text };
    setFile(f);
    await runPreview(accountId, f);
  }

  function changeAccount(id: number) {
    setAccountId(id);
    if (file) void runPreview(id, file);
  }

  function toggle(line: number) {
    setTicked((prev) => {
      const next = new Set(prev);
      if (next.has(line)) next.delete(line);
      else next.add(line);
      return next;
    });
  }

  function reset() {
    setFile(null);
    setPreview(null);
    setTicked(new Set());
    setFileError(null);
  }

  async function doImport() {
    if (!preview || busy) return;
    const rows = preview.rows.filter((r) => r.status !== "duplicate" && ticked.has(r.line)).map(toImportRow);
    if (rows.length === 0) return;
    setBusy(true);
    try {
      const r = await api.commitImport(preview.account_id, rows);
      toast.success(
        `Imported ${r.imported} transaction${r.imported === 1 ? "" : "s"}` +
          (r.skipped ? ` (${r.skipped} already there).` : "."),
      );
      setDone({ accountId: preview.account_id, ...r });
      reset();
    } catch (e) {
      toast.error(`Import failed, nothing was saved: ${errorMessage(e)}`);
    } finally {
      setBusy(false);
    }
  }

  const counts = useMemo(() => {
    const c: Record<ImportStatus, number> = { new: 0, duplicate: 0, possible_match: 0, before_opening: 0 };
    for (const r of preview?.rows ?? []) c[r.status] += 1;
    return c;
  }, [preview]);

  const catById = useMemo(() => categoryIndex(categories), [categories]);
  const tickedCount = (preview?.rows ?? []).filter((r) => r.status !== "duplicate" && ticked.has(r.line)).length;
  const accountName = accounts?.find((a) => a.id === (done?.accountId ?? accountId))?.name ?? "";

  if (loadError) {
    return (
      <div className="page-error" role="alert">
        Couldn't load accounts: {loadError}
      </div>
    );
  }
  if (accounts === null) return <p className="muted">Loading…</p>;
  if (accounts.length === 0) {
    return <div className="table-empty">Add an account first, then import its bank export.</div>;
  }

  const columns: Column<ImportPreviewRow>[] = [
    {
      key: "pick",
      header: "Import",
      width: "56px",
      render: (r) => (
        <input
          type="checkbox"
          aria-label={`Import line ${r.line}: ${r.payee ?? r.description}`}
          checked={r.status !== "duplicate" && ticked.has(r.line)}
          disabled={r.status === "duplicate" || busy}
          onChange={() => toggle(r.line)}
        />
      ),
    },
    { key: "date", header: "Date", width: "110px", render: (r) => formatDate(r.date) },
    {
      key: "payee",
      header: "Payee",
      render: (r) => (
        <div className="cell-stack">
          <span className="cell-title">{r.payee ?? r.description}</span>
          {r.payee && <span className="cell-sub">{r.description}</span>}
        </div>
      ),
    },
    {
      key: "category",
      header: "Category",
      width: "140px",
      render: (r) => {
        const c = r.category_id === null ? undefined : catById.get(r.category_id);
        return c ? <span title={`Set by a rule: ${c.group} › ${c.name}`}>{c.name}</span> : <span className="muted">—</span>;
      },
    },
    { key: "amount", header: "Amount", width: "110px", money: (r) => r.amount },
    {
      key: "balance",
      header: "Bank balance",
      width: "120px",
      align: "right",
      render: (r) =>
        r.balance === null ? (
          <span className="muted">—</span>
        ) : (
          <span className={`money${r.balance < 0 ? " negative" : ""}`}>{formatCents(r.balance)}</span>
        ),
    },
    {
      key: "status",
      header: "Status",
      width: "150px",
      render: (r) => (
        <span className={`badge status-${r.status}`} title={STATUS_HINTS[r.status]}>
          {STATUS_LABELS[r.status]}
        </span>
      ),
    },
  ];

  const mismatch =
    preview && preview.bank_balance !== null && preview.bank_balance !== preview.balance_after;

  return (
    <div className="import-page">
      <p className="muted">
        In Bank of Melbourne internet banking, export the account's transactions as CSV, then choose the file
        here. Nothing is saved until you press Import.
      </p>

      <div className="toolbar filters">
        <SelectField
          label="Import into"
          value={accountId === null ? "" : String(accountId)}
          onChange={(e) => changeAccount(Number(e.currentTarget.value))}
          disabled={busy}
        >
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </SelectField>
        <div className="import-file-pick">
          <Button variant="secondary" onClick={() => fileInput.current?.click()} disabled={busy}>
            <FileUp size={16} aria-hidden="true" /> {file ? "Choose another file…" : "Choose CSV file…"}
          </Button>
          <input
            ref={fileInput}
            type="file"
            accept=".csv,text/csv"
            className="sr-only"
            aria-label="Bank export file"
            onChange={chooseFile}
          />
        </div>
        {file && <span className="muted import-file">{file.name}</span>}
      </div>

      {fileError && (
        <p className="field-error" role="alert">
          {fileError}
        </p>
      )}

      {done && (
        <div className="import-done" role="status">
          <span>
            Imported {done.imported} transaction{done.imported === 1 ? "" : "s"} into {accountName}.
          </span>
          <Button variant="secondary" onClick={() => onViewTransactions(done.accountId)}>
            View in Transactions
          </Button>
        </div>
      )}

      {preview && (
        <>
          <p className="import-summary">
            {counts.new} new · {counts.duplicate} already imported · {counts.possible_match} possible match
            {counts.possible_match === 1 ? "" : "es"} · {counts.before_opening} before opening date
          </p>

          {mismatch && (
            <div className="warning" role="note">
              With the new rows, Plutus will show <strong>{formatCents(preview.balance_after)}</strong> for{" "}
              {accountName}, but the bank file says <strong>{formatCents(preview.bank_balance!)}</strong>. Check the
              account's opening balance and date, or whether earlier transactions are missing.
            </div>
          )}
          {preview.balance_breaks.length > 0 && (
            <div className="warning" role="note">
              The bank balances don't add up near line{preview.balance_breaks.length === 1 ? "" : "s"}{" "}
              {preview.balance_breaks.join(", ")}. The export may be missing rows there.
            </div>
          )}

          <DataTable
            columns={columns}
            rows={preview.rows}
            rowKey={(r) => r.line}
            caption="Rows in the bank file"
            rowClassName={(r) => (r.status === "duplicate" ? "archived" : undefined)}
          />

          <div className="toolbar">
            <span className="spacer" />
            <Button variant="secondary" onClick={reset} disabled={busy}>
              Cancel
            </Button>
            <Button onClick={doImport} disabled={tickedCount === 0} busy={busy} busyLabel="Importing…">
              {tickedCount === 0
                ? "Nothing to import"
                : `Import ${tickedCount} transaction${tickedCount === 1 ? "" : "s"}`}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
