import { ChangeEvent, useEffect, useMemo, useRef, useState } from "react";
import { FileUp } from "lucide-react";
import { Button, DataTable, SelectField, TextField, useToast } from "../components";
import type { Column } from "../components";
import {
  Account,
  api,
  errorMessage,
  CategoryGroup,
  DateFormat,
  ImportFileShape,
  ImportMapping,
  ImportPreset,
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

/** Bank of Melbourne's exact column order; anything else needs a mapping. */
const BOM_HEADERS = ["date", "description", "debit", "credit", "balance"];

function isBankOfMelbourne(headers: string[]): boolean {
  return (
    headers.length === BOM_HEADERS.length &&
    headers.every((h, i) => h.trim().toLowerCase() === BOM_HEADERS[i])
  );
}

/** OFX/QFX statements start with an OFX header or an <OFX> tag. */
function looksLikeOfx(text: string): boolean {
  const head = text.replace(/^\uFEFF/, "");
  return head.includes("OFXHEADER") || head.includes("<OFX>") || head.includes("<STMTTRN>");
}

/** QIF files carry a `!Type:` section header near the top. */
function looksLikeQif(text: string): boolean {
  return text
    .replace(/^\uFEFF/, "")
    .split(/\r?\n/)
    .slice(0, 20)
    .some((line) => line.trim().startsWith("!Type:"));
}

const DATE_FORMATS: { value: DateFormat; label: string; example: string }[] = [
  { value: "mdy", label: "Month first", example: "11/20/2026" },
  { value: "dmy", label: "Day first", example: "20/11/2026" },
  { value: "ymd", label: "Year first", example: "2026-11-20" },
];

/** An empty mapping for a file whose columns couldn't be guessed. */
function blankMapping(): ImportMapping {
  return {
    date: "",
    date_format: "mdy",
    description: "",
    payee: null,
    amount: null,
    debit: null,
    credit: null,
    balance: null,
  };
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
  // Generic CSV: the header row and the owner's column mapping.
  const [shape, setShape] = useState<ImportFileShape | null>(null);
  const [mapping, setMapping] = useState<ImportMapping | null>(null);
  const [mappingError, setMappingError] = useState<string | null>(null);
  const [presets, setPresets] = useState<ImportPreset[]>([]);
  const [presetId, setPresetId] = useState("");
  const [presetName, setPresetName] = useState("");
  const [savingPreset, setSavingPreset] = useState(false);
  // QIF: dates are ambiguous, so the owner picks the format before previewing.
  const [qifText, setQifText] = useState<string | null>(null);
  const [qifFormat, setQifFormat] = useState<DateFormat>("mdy");

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

  async function runGenericPreview(acct: number, f: { name: string; text: string }, m: ImportMapping) {
    setBusy(true);
    setFileError(null);
    setMappingError(null);
    try {
      const p = await api.previewGenericImport(acct, f.text, m);
      setPreview(p);
      setTicked(new Set(p.rows.filter((r) => r.status === "new").map((r) => r.line)));
    } catch (e) {
      const { field, general } = splitError(e);
      setPreview(null);
      // A mapping mistake belongs next to the mapping form, not the file picker.
      if (field.mapping) setMappingError(field.mapping);
      else setFileError(field.file ?? field.account_id ?? general ?? "Couldn't read the file.");
    } finally {
      setBusy(false);
    }
  }

  async function runOfxPreview(acct: number, f: { name: string; text: string }) {
    setBusy(true);
    setFileError(null);
    try {
      const p = await api.previewOfxImport(acct, f.text);
      setPreview(p);
      setTicked(new Set(p.rows.filter((r) => r.status === "new").map((r) => r.line)));
    } catch (e) {
      const { field, general } = splitError(e);
      setPreview(null);
      setFileError(field.file ?? field.account_id ?? general ?? "Couldn't read the statement.");
    } finally {
      setBusy(false);
    }
  }

  async function runQifPreview(acct: number, f: { name: string; text: string }, format: DateFormat) {
    setBusy(true);
    setFileError(null);
    try {
      const p = await api.previewQifImport(acct, f.text, format);
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
    setPreview(null);
    setShape(null);
    setMapping(null);
    setMappingError(null);
    setPresetId("");
    setPresetName("");
    setQifText(null);
    let text: string;
    try {
      text = await readText(picked);
    } catch (err) {
      setFileError(`Couldn't open ${picked.name}: ${errorMessage(err)}`);
      return;
    }
    const f = { name: picked.name, text };
    setFile(f);

    // OFX/QFX and QIF are self-describing; everything else is CSV, where the
    // header row decides which importer handles the file.
    if (looksLikeOfx(text)) {
      await runOfxPreview(accountId, f);
      return;
    }
    if (looksLikeQif(text)) {
      setQifText(text);
      setShape(null);
      setMapping(null);
      setFileError(null);
      return;
    }

    setBusy(true);
    setFileError(null);
    let fileShape: ImportFileShape;
    try {
      fileShape = await api.importShape(text);
    } catch (err) {
      const { field, general } = splitError(err);
      setFileError(field.file ?? general ?? "Couldn't read the file.");
      setBusy(false);
      return;
    }
    setBusy(false);

    if (isBankOfMelbourne(fileShape.headers)) {
      await runPreview(accountId, f);
      return;
    }
    setShape(fileShape);
    setMapping(fileShape.mapping.date ? fileShape.mapping : blankMapping());
    setFileError(null);
    try {
      setPresets(await api.listImportPresets());
    } catch {
      setPresets([]);
    }
  }

  function changeAccount(id: number) {
    setAccountId(id);
    setDone(null);
    if (file && qifText) void runQifPreview(id, file, qifFormat);
    else if (file && mapping && shape) void runGenericPreview(id, file, mapping);
    else if (file && !shape && !qifText) void runPreview(id, file);
  }

  function setMapField(field: keyof ImportMapping, value: string | null) {
    setMapping((m) => (m ? { ...m, [field]: value } : m));
    setMappingError(null);
  }

  function applyPreset(id: string) {
    setPresetId(id);
    const p = presets.find((x) => String(x.id) === id);
    if (p) {
      setMapping({ ...p.mapping });
      setMappingError(null);
    }
  }

  async function savePreset() {
    if (!mapping) return;
    const name = presetName.trim();
    if (!name) {
      setMappingError("Give the preset a name.");
      return;
    }
    setSavingPreset(true);
    try {
      const saved = await api.saveImportPreset(name, mapping);
      setPresets((prev) => {
        const next = prev.filter((p) => p.id !== saved.id);
        return [...next, saved].sort((a, b) => a.name.localeCompare(b.name));
      });
      setPresetId(String(saved.id));
      setPresetName("");
      toast.success(`Saved preset “${saved.name}”.`);
    } catch (e) {
      const { field, general } = splitError(e);
      setMappingError(field.mapping ?? field.name ?? general ?? "Couldn't save the preset.");
    } finally {
      setSavingPreset(false);
    }
  }

  async function deletePreset() {
    const p = presets.find((x) => String(x.id) === presetId);
    if (!p) return;
    try {
      await api.deleteImportPreset(p.id);
      setPresets((prev) => prev.filter((x) => x.id !== p.id));
      setPresetId("");
      toast.success(`Deleted preset “${p.name}”.`);
    } catch (e) {
      toast.error(`Couldn't delete the preset: ${errorMessage(e)}`);
    }
  }

  function previewMapped() {
    if (!file || !mapping || accountId === null) return;
    void runGenericPreview(accountId, file, mapping);
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
    setShape(null);
    setMapping(null);
    setMappingError(null);
    setPresetId("");
    setPresetName("");
    setQifText(null);
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
        Export the account's transactions as CSV from your bank, then choose the file here. Bank of
        Melbourne exports are recognised automatically; for any other bank, say which column holds what.
        Nothing is saved until you press Import.
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

      {qifText && !preview && (
        <div className="import-mapping">
          <h2>QIF dates</h2>
          <p className="muted">
            {file?.name} is a QIF file. QIF writes dates day/month in whatever order the exporting
            program used, so tell Plutus which way to read them.
          </p>
          <div className="toolbar">
            <SelectField
              label="Date format"
              hint={DATE_FORMATS.find((f) => f.value === qifFormat)?.example}
              value={qifFormat}
              onChange={(e) => setQifFormat(e.currentTarget.value as DateFormat)}
              disabled={busy}
            >
              {DATE_FORMATS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label} ({f.example})
                </option>
              ))}
            </SelectField>
            <Button
              onClick={() => {
                if (file && accountId !== null) void runQifPreview(accountId, file, qifFormat);
              }}
              busy={busy}
              busyLabel="Reading…"
            >
              Preview
            </Button>
          </div>
        </div>
      )}

      {shape && mapping && !preview && (
        <div className="import-mapping">
          <h2>Map the columns</h2>
          <p className="muted">
            {file?.name} doesn't look like a Bank of Melbourne export. Tell Plutus which column holds
            what; nothing is saved until you import. Save the mapping as a preset so next month's file
            only takes one click.
          </p>

          {presets.length > 0 && (
            <div className="toolbar">
              <SelectField
                label="Saved preset"
                value={presetId}
                onChange={(e) => applyPreset(e.currentTarget.value)}
                disabled={busy}
              >
                <option value="">— None —</option>
                {presets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </SelectField>
              {presetId && (
                <Button variant="secondary" onClick={deletePreset} disabled={busy}>
                  Delete preset
                </Button>
              )}
            </div>
          )}

          <div className="mapping-grid">
            <SelectField
              label="Date"
              value={mapping.date}
              onChange={(e) => setMapField("date", e.currentTarget.value)}
              disabled={busy}
            >
              <option value="">— Choose —</option>
              {shape.headers.map((h) => (
                <option key={h} value={h}>
                  {h}
                </option>
              ))}
            </SelectField>

            <SelectField
              label="Date format"
              hint={DATE_FORMATS.find((f) => f.value === mapping.date_format)?.example}
              value={mapping.date_format}
              onChange={(e) => setMapField("date_format", e.currentTarget.value)}
              disabled={busy}
            >
              {DATE_FORMATS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label} ({f.example})
                </option>
              ))}
            </SelectField>

            <SelectField
              label="Description"
              value={mapping.description}
              onChange={(e) => setMapField("description", e.currentTarget.value)}
              disabled={busy}
            >
              <option value="">— Choose —</option>
              {shape.headers.map((h) => (
                <option key={h} value={h}>
                  {h}
                </option>
              ))}
            </SelectField>

            <SelectField
              label="Payee (optional)"
              hint="Leave empty to guess the merchant from the description."
              value={mapping.payee ?? ""}
              onChange={(e) => setMapField("payee", e.currentTarget.value || null)}
              disabled={busy}
            >
              <option value="">— None —</option>
              {shape.headers.map((h) => (
                <option key={h} value={h}>
                  {h}
                </option>
              ))}
            </SelectField>

            <SelectField
              label="Amounts"
              hint="One signed column, or separate money out and money in columns."
              value={mapping.debit !== null || mapping.credit !== null ? "pair" : "single"}
              onChange={(e) => {
                const pair = e.currentTarget.value === "pair";
                setMapping((m) =>
                  m
                    ? {
                        ...m,
                        amount: pair ? null : m.amount,
                        debit: pair ? m.debit : null,
                        credit: pair ? m.credit : null,
                      }
                    : m,
                );
                setMappingError(null);
              }}
              disabled={busy}
            >
              <option value="single">One amount column</option>
              <option value="pair">Debit and credit columns</option>
            </SelectField>

            {mapping.debit !== null || mapping.credit !== null ? (
              <>
                <SelectField
                  label="Money out"
                  value={mapping.debit ?? ""}
                  onChange={(e) => setMapField("debit", e.currentTarget.value || null)}
                  disabled={busy}
                >
                  <option value="">— None —</option>
                  {shape.headers.map((h) => (
                    <option key={h} value={h}>
                      {h}
                    </option>
                  ))}
                </SelectField>
                <SelectField
                  label="Money in"
                  value={mapping.credit ?? ""}
                  onChange={(e) => setMapField("credit", e.currentTarget.value || null)}
                  disabled={busy}
                >
                  <option value="">— None —</option>
                  {shape.headers.map((h) => (
                    <option key={h} value={h}>
                      {h}
                    </option>
                  ))}
                </SelectField>
              </>
            ) : (
              <SelectField
                label="Amount"
                hint="Signed: negative is money out."
                value={mapping.amount ?? ""}
                onChange={(e) => setMapField("amount", e.currentTarget.value || null)}
                disabled={busy}
              >
                <option value="">— Choose —</option>
                {shape.headers.map((h) => (
                  <option key={h} value={h}>
                    {h}
                  </option>
                ))}
              </SelectField>
            )}

            <SelectField
              label="Bank balance (optional)"
              hint="Used to check the file against the bank."
              value={mapping.balance ?? ""}
              onChange={(e) => setMapField("balance", e.currentTarget.value || null)}
              disabled={busy}
            >
              <option value="">— None —</option>
              {shape.headers.map((h) => (
                <option key={h} value={h}>
                  {h}
                </option>
              ))}
            </SelectField>
          </div>

          {mappingError && (
            <p className="field-error" role="alert">
              {mappingError}
            </p>
          )}

          <div className="toolbar">
            <TextField
              label="Save as preset"
              placeholder="e.g. ANZ everyday"
              value={presetName}
              onChange={(e) => setPresetName(e.currentTarget.value)}
              disabled={busy || savingPreset}
            />
            <Button variant="secondary" onClick={savePreset} disabled={busy || savingPreset || !presetName.trim()}>
              {savingPreset ? "Saving…" : "Save preset"}
            </Button>
            <span className="spacer" />
            <Button variant="secondary" onClick={reset} disabled={busy}>
              Cancel
            </Button>
            <Button onClick={previewMapped} disabled={busy} busy={busy} busyLabel="Reading…">
              Preview
            </Button>
          </div>
        </div>
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
            <Button
              variant="secondary"
              onClick={() => (shape || qifText ? setPreview(null) : reset())}
              disabled={busy}
            >
              {shape ? "Back to mapping" : qifText ? "Back to dates" : "Cancel"}
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
