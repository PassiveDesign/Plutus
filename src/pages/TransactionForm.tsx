import { FormEvent, useState } from "react";
import { Button, Modal, SelectField, TextField, useToast } from "../components";
import { Account, api, errorMessage, splitError, Transaction, TransactionInput } from "../lib/api";
import { todayIso } from "../lib/dates";
import { plainCents } from "../lib/money";

export type TxVerb = "added" | "updated" | "deleted";

type Props = {
  /** `null` = adding a new transaction. */
  transaction: Transaction | null;
  /** Every account (archived included); only active ones are offered. */
  accounts: Account[];
  /** Preselected account for new transactions (e.g. the current filter). */
  defaultAccountId: number | null;
  onClose: () => void;
  onDone: (verb: TxVerb) => void;
};

const FIELDS = new Set(["account_id", "date", "amount", "payee", "description", "memo"]);

const blank = (s: string): string | null => (s.trim() === "" ? null : s);

export function TransactionForm({ transaction: tx, accounts, defaultAccountId, onClose, onDone }: Props) {
  const toast = useToast();
  // Active accounts, plus the transaction's own account if it was archived.
  const choices = accounts.filter((a) => a.archived_at === null || a.id === tx?.account_id);
  const initialAccount = tx?.account_id ?? defaultAccountId ?? choices[0]?.id ?? null;

  const [accountId, setAccountId] = useState<number | null>(initialAccount);
  const [date, setDate] = useState(tx?.date ?? todayIso());
  const [direction, setDirection] = useState<"out" | "in">(tx && tx.amount > 0 ? "in" : "out");
  const [amountText, setAmountText] = useState(tx ? plainCents(Math.abs(tx.amount)) : "");
  const [payee, setPayee] = useState(tx?.payee ?? "");
  const [description, setDescription] = useState(tx?.description ?? "");
  const [memo, setMemo] = useState(tx?.memo ?? "");
  const [cleared, setCleared] = useState(tx?.cleared ?? false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  async function save(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    if (accountId === null) {
      setErrors({ account_id: "Choose an account." });
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
    const input: TransactionInput = {
      account_id: accountId,
      date,
      amount: direction === "out" && cents !== 0 ? -cents : cents,
      payee: blank(payee),
      description: blank(description),
      memo: blank(memo),
      cleared,
    };

    try {
      if (tx) await api.updateTransaction(tx.id, input);
      else await api.addTransaction(input);
      onDone(tx ? "updated" : "added");
    } catch (err) {
      const { field, general } = splitError(err);
      const inline = Object.fromEntries(Object.entries(field).filter(([k]) => FIELDS.has(k)));
      if (Object.keys(inline).length > 0) setErrors(inline);
      else toast.error(general ?? Object.values(field)[0] ?? "Couldn't save the transaction.");
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
      await api.deleteTransaction(tx.id);
      onDone("deleted");
    } catch (err) {
      toast.error(`Couldn't delete: ${errorMessage(err)}`);
      setBusy(false);
      setConfirmDelete(false);
    }
  }

  return (
    <Modal
      open
      title={tx ? "Edit transaction" : "Add transaction"}
      onClose={onClose}
      footer={
        <>
          {tx && (
            <Button variant={confirmDelete ? "danger" : "ghost"} onClick={remove} disabled={busy}>
              {confirmDelete ? "Really delete?" : "Delete"}
            </Button>
          )}
          <span className="spacer" />
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" form="tx-form" busy={busy} busyLabel="Saving…">
            {tx ? "Save" : "Add transaction"}
          </Button>
        </>
      }
    >
      <form id="tx-form" className="form-grid" onSubmit={save} noValidate>
        <SelectField
          label="Account"
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

        <fieldset className="radio-row">
          <legend>Direction</legend>
          <div className="radio-options">
          <label className="check">
            <input
              type="radio"
              name="direction"
              value="out"
              checked={direction === "out"}
              onChange={() => setDirection("out")}
            />
            Money out
          </label>
          <label className="check">
            <input
              type="radio"
              name="direction"
              value="in"
              checked={direction === "in"}
              onChange={() => setDirection("in")}
            />
            Money in
          </label>
          </div>
        </fieldset>

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
        <TextField label="Memo" value={memo} onChange={(e) => setMemo(e.currentTarget.value)} error={errors.memo} />
        <label className="check">
          <input type="checkbox" checked={cleared} onChange={(e) => setCleared(e.currentTarget.checked)} />
          Cleared (matches the bank statement)
        </label>
      </form>
    </Modal>
  );
}
