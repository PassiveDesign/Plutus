import { FormEvent, useState } from "react";
import { Button, Modal, SelectField, TextField, useToast } from "../components";
import { CategorySelect } from "./CategorySelect";
import {
  Account,
  api,
  CategoryGroup,
  errorMessage,
  splitError,
  Transaction,
  TransactionInput,
  TransferInput,
} from "../lib/api";
import { todayIso } from "../lib/dates";
import { plainCents } from "../lib/money";

export type TxVerb = "added" | "updated" | "deleted";

type Kind = "out" | "in" | "transfer";

type Props = {
  /** `null` = adding a new transaction. */
  transaction: Transaction | null;
  /** Every account (archived included); only active ones are offered. */
  accounts: Account[];
  /** Full category tree (archived included). */
  categories: CategoryGroup[];
  /** Preselected account for new transactions (e.g. the current filter). */
  defaultAccountId: number | null;
  onClose: () => void;
  onDone: (verb: TxVerb, what: "Transaction" | "Transfer") => void;
};

/** Backend field -> form field. Transfer legs report `from_account` /
 *  `to_account`; the "from" side uses the main Account picker. */
const FIELD_ALIASES: Record<string, string> = { from_account: "account_id" };
const FIELDS = new Set(["account_id", "to_account", "date", "amount", "payee", "description", "memo", "category_id"]);

const blank = (s: string): string | null => (s.trim() === "" ? null : s);

export function TransactionForm({ transaction: tx, accounts, categories, defaultAccountId, onClose, onDone }: Props) {
  const toast = useToast();
  const isTransfer = tx?.transfer_id != null;

  // For an existing transfer, "from" is the outgoing leg's account whichever
  // leg was clicked.
  const initialFrom = isTransfer
    ? tx!.amount < 0
      ? tx!.account_id
      : tx!.transfer_account_id
    : (tx?.account_id ?? null);
  const initialTo = isTransfer ? (tx!.amount < 0 ? tx!.transfer_account_id : tx!.account_id) : null;

  // Active accounts, plus any account this transaction already uses.
  const used = new Set([tx?.account_id, tx?.transfer_account_id].filter((x): x is number => x != null));
  const choices = accounts.filter((a) => a.archived_at === null || used.has(a.id));
  const firstFrom = initialFrom ?? defaultAccountId ?? choices[0]?.id ?? null;

  const [kind, setKind] = useState<Kind>(isTransfer ? "transfer" : tx && tx.amount > 0 ? "in" : "out");
  const [accountId, setAccountId] = useState<number | null>(firstFrom);
  const [toAccountId, setToAccountId] = useState<number | null>(
    initialTo ?? choices.find((a) => a.id !== firstFrom)?.id ?? null,
  );
  const [date, setDate] = useState(tx?.date ?? todayIso());
  const [amountText, setAmountText] = useState(tx ? plainCents(Math.abs(tx.amount)) : "");
  const [payee, setPayee] = useState(tx?.payee ?? "");
  const [description, setDescription] = useState(tx?.description ?? "");
  const [memo, setMemo] = useState(tx?.memo ?? "");
  const [cleared, setCleared] = useState(tx?.cleared ?? false);
  const [categoryId, setCategoryId] = useState<number | null>(tx?.category_id ?? null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const transfer = kind === "transfer";
  const what = transfer ? "Transfer" : "Transaction";

  function showError(err: unknown) {
    const { field, general } = splitError(err);
    const inline = Object.fromEntries(
      Object.entries(field)
        .map(([k, v]) => [FIELD_ALIASES[k] ?? k, v] as const)
        .filter(([k]) => FIELDS.has(k)),
    );
    if (Object.keys(inline).length > 0) setErrors(inline);
    else toast.error(general ?? Object.values(field)[0] ?? `Couldn't save the ${what.toLowerCase()}.`);
  }

  async function save(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    if (accountId === null) {
      setErrors({ account_id: "Choose an account." });
      return;
    }
    if (transfer && toAccountId === null) {
      setErrors({ to_account: "Choose an account to transfer to." });
      return;
    }
    setBusy(true);
    setErrors({});

    let cents: number;
    try {
      cents = Math.abs(await api.parseMoney(amountText));
    } catch (err) {
      setErrors({ amount: errorMessage(err) });
      setBusy(false);
      return;
    }

    try {
      if (transfer) {
        const input: TransferInput = {
          from_account: accountId,
          to_account: toAccountId!,
          date,
          amount: cents,
          memo: blank(memo),
        };
        if (tx?.transfer_id != null) await api.updateTransfer(tx.transfer_id, input);
        else await api.createTransfer(input);
      } else {
        const input: TransactionInput = {
          account_id: accountId,
          date,
          amount: kind === "out" && cents !== 0 ? -cents : cents,
          payee: blank(payee),
          description: blank(description),
          memo: blank(memo),
          cleared,
          category_id: categoryId,
        };
        if (tx) await api.updateTransaction(tx.id, input);
        else await api.addTransaction(input);
      }
      onDone(tx ? "updated" : "added", what);
    } catch (err) {
      showError(err);
      setBusy(false);
    }
  }

  async function remove() {
    if (!tx || busy) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    setBusy(true);
    try {
      await api.deleteTransaction(tx.id); // removes both legs of a transfer
      onDone("deleted", what);
    } catch (err) {
      toast.error(`Couldn't delete: ${errorMessage(err)}`);
      setBusy(false);
      setConfirmDelete(false);
    }
  }

  // New: all three; existing ordinary: out/in; existing transfer: fixed.
  const kindOptions: { value: Kind; label: string }[] = isTransfer
    ? []
    : [
        { value: "out", label: "Money out" },
        { value: "in", label: "Money in" },
        ...(tx ? [] : [{ value: "transfer" as const, label: "Transfer" }]),
      ];

  return (
    <Modal
      open
      title={tx ? `Edit ${what.toLowerCase()}` : `Add ${transfer ? "transfer" : "transaction"}`}
      onClose={onClose}
      footer={
        <>
          {tx && (
            <Button variant={confirmDelete ? "danger" : "ghost"} onClick={remove} disabled={busy}>
              {confirmDelete ? (isTransfer ? "Really delete both sides?" : "Really delete?") : "Delete"}
            </Button>
          )}
          <span className="spacer" />
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" form="tx-form" busy={busy} busyLabel="Saving…">
            {tx ? "Save" : transfer ? "Add transfer" : "Add transaction"}
          </Button>
        </>
      }
    >
      <form id="tx-form" className="form-grid" onSubmit={save} noValidate>
        {kindOptions.length > 0 && (
          <fieldset className="radio-row">
            <legend>Type</legend>
            <div className="radio-options">
              {kindOptions.map((o) => (
                <label key={o.value} className="check">
                  <input
                    type="radio"
                    name="kind"
                    value={o.value}
                    checked={kind === o.value}
                    onChange={() => setKind(o.value)}
                  />
                  {o.label}
                </label>
              ))}
            </div>
          </fieldset>
        )}

        <div className={transfer ? "form-row" : undefined}>
          <SelectField
            label={transfer ? "From account" : "Account"}
            value={accountId === null ? "" : String(accountId)}
            onChange={(e) => setAccountId(e.currentTarget.value === "" ? null : Number(e.currentTarget.value))}
            error={errors.account_id}
          >
            {accountId === null && <option value="">Choose an account</option>}
            {choices.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
                {a.archived_at ? " (archived)" : ""}
              </option>
            ))}
          </SelectField>
          {transfer && (
            <SelectField
              label="To account"
              value={toAccountId === null ? "" : String(toAccountId)}
              onChange={(e) =>
                setToAccountId(e.currentTarget.value === "" ? null : Number(e.currentTarget.value))
              }
              error={errors.to_account}
            >
              {toAccountId === null && <option value="">Choose an account</option>}
              {choices.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                  {a.archived_at ? " (archived)" : ""}
                </option>
              ))}
            </SelectField>
          )}
        </div>

        <div className="form-row">
          <TextField
            label="Amount"
            value={amountText}
            onChange={(e) => setAmountText(e.currentTarget.value)}
            error={errors.amount}
            placeholder="0.00"
            inputMode="decimal"
            autoFocus={!tx}
          />
          <TextField
            label="Date"
            type="date"
            value={date}
            onChange={(e) => setDate(e.currentTarget.value)}
            error={errors.date}
          />
        </div>
        {!transfer && (
          <>
            <TextField
              label="Payee"
              value={payee}
              onChange={(e) => setPayee(e.currentTarget.value)}
              error={errors.payee}
              placeholder="e.g. Woolworths"
            />
            <TextField
              label="Bank description"
              value={description}
              onChange={(e) => setDescription(e.currentTarget.value)}
              error={errors.description}
              hint="As it appears on your statement. Optional if you enter a payee."
            />
          </>
        )}
        {!transfer && (
          <CategorySelect
            groups={categories}
            value={categoryId}
            onChange={setCategoryId}
            keep={tx?.category_id ?? null}
            error={errors.category_id}
          />
        )}
        <TextField label="Memo" value={memo} onChange={(e) => setMemo(e.currentTarget.value)} error={errors.memo} />
        {transfer ? (
          <p className="field-hint">
            Recorded as money out of the first account and in to the second, on the same date.
          </p>
        ) : (
          <label className="check">
            <input type="checkbox" checked={cleared} onChange={(e) => setCleared(e.currentTarget.checked)} />
            Cleared (matches the bank statement)
          </label>
        )}
      </form>
    </Modal>
  );
}
