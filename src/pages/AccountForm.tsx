import { FormEvent, useState } from "react";
import { Button, Modal, SelectField, TextField, useToast } from "../components";
import { ACCOUNT_GROUPS, isLiability, KIND_LABELS } from "../lib/accounts";
import { Account, AccountInput, AccountKind, api, errorMessage, splitError } from "../lib/api";
import { plainCents } from "../lib/money";

export type SavedVerb = "added" | "updated" | "archived" | "restored";

type Props = {
  /** `null` = adding a new account. */
  account: Account | null;
  onClose: () => void;
  onSaved: (account: Account, verb: SavedVerb) => void;
};

/** Form fields that can show a backend validation error inline. */
const FIELDS = new Set([
  "name",
  "kind",
  "institution",
  "bsb",
  "account_number",
  "opening_balance",
  "opening_date",
  "notes",
]);

function todayIso(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

const blank = (s: string): string | null => (s.trim() === "" ? null : s);

/** Liabilities are stored negative but entered as a positive "amount owed". */
function toDisplayCents(kind: AccountKind, cents: number): number {
  return isLiability(kind) && cents !== 0 ? -cents : cents;
}

export function AccountForm({ account, onClose, onSaved }: Props) {
  const toast = useToast();
  const [name, setName] = useState(account?.name ?? "");
  const [kind, setKind] = useState<AccountKind>(account?.kind ?? "offset");
  const [institution, setInstitution] = useState(account?.institution ?? "");
  const [bsb, setBsb] = useState(account?.bsb ?? "");
  const [accountNumber, setAccountNumber] = useState(account?.account_number ?? "");
  const [balanceText, setBalanceText] = useState(
    account ? plainCents(toDisplayCents(account.kind, account.opening_balance)) : "0.00",
  );
  const [openingDate, setOpeningDate] = useState(account?.opening_date ?? todayIso());
  const [notes, setNotes] = useState(account?.notes ?? "");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);

  const liability = isLiability(kind);
  const isNew = account === null;

  async function save(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    setBusy(true);
    setErrors({});

    let cents: number;
    try {
      cents = await api.parseMoney(balanceText.trim() === "" ? "0" : balanceText);
    } catch (err) {
      setErrors({ opening_balance: errorMessage(err) });
      setBusy(false);
      return;
    }
    if (liability && cents !== 0) cents = -cents;

    const input: AccountInput = {
      name,
      kind,
      institution: blank(institution),
      bsb: blank(bsb),
      account_number: blank(accountNumber),
      opening_balance: cents,
      opening_date: openingDate,
      // The offset-loan picker comes later; keep an existing link while the
      // account stays an offset, drop it otherwise.
      offset_for: kind === "offset" ? (account?.offset_for ?? null) : null,
      notes: blank(notes),
    };

    try {
      const saved = isNew
        ? await api.createAccount(input)
        : await api.updateAccount(account.id, input);
      onSaved(saved, isNew ? "added" : "updated");
    } catch (err) {
      const { field, general } = splitError(err);
      const inline = Object.fromEntries(Object.entries(field).filter(([k]) => FIELDS.has(k)));
      if (Object.keys(inline).length > 0) {
        setErrors(inline);
      } else {
        toast.error(general ?? Object.values(field)[0] ?? "Couldn't save the account.");
      }
      setBusy(false);
    }
  }

  async function toggleArchived() {
    if (!account || busy) return;
    setBusy(true);
    const archive = account.archived_at === null;
    try {
      const saved = await api.setAccountArchived(account.id, archive);
      onSaved(saved, archive ? "archived" : "restored");
    } catch (err) {
      const { field, general } = splitError(err);
      toast.error(general ?? Object.values(field)[0]);
      setBusy(false);
    }
  }

  return (
    <Modal
      open
      title={isNew ? "Add account" : `Edit ${account.name}`}
      onClose={onClose}
      footer={
        <>
          {!isNew && (
            <Button variant="ghost" onClick={toggleArchived} disabled={busy}>
              {account.archived_at ? "Restore" : "Archive"}
            </Button>
          )}
          <span className="spacer" />
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" form="account-form" busy={busy} busyLabel="Saving…">
            {isNew ? "Add account" : "Save"}
          </Button>
        </>
      }
    >
      <form id="account-form" className="form-grid" onSubmit={save} noValidate>
        <TextField
          label="Name"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          error={errors.name}
          placeholder="e.g. Everyday Offset"
        />
        <SelectField
          label="Type"
          value={kind}
          onChange={(e) => setKind(e.currentTarget.value as AccountKind)}
          error={errors.kind}
        >
          {ACCOUNT_GROUPS.map((g) => (
            <optgroup key={g.id} label={g.label}>
              {g.kinds.map((k) => (
                <option key={k} value={k}>
                  {KIND_LABELS[k]}
                </option>
              ))}
            </optgroup>
          ))}
        </SelectField>
        <TextField
          label="Institution"
          value={institution}
          onChange={(e) => setInstitution(e.currentTarget.value)}
          error={errors.institution}
          placeholder="e.g. Bank of Melbourne"
        />
        <div className="form-row">
          <TextField
            label="BSB"
            value={bsb}
            onChange={(e) => setBsb(e.currentTarget.value)}
            error={errors.bsb}
            placeholder="063-000"
            inputMode="numeric"
          />
          <TextField
            label="Account number"
            value={accountNumber}
            onChange={(e) => setAccountNumber(e.currentTarget.value)}
            error={errors.account_number}
            inputMode="numeric"
          />
        </div>
        <div className="form-row">
          <TextField
            label={liability ? "Opening amount owed" : "Opening balance"}
            value={balanceText}
            onChange={(e) => setBalanceText(e.currentTarget.value)}
            error={errors.opening_balance}
            hint={liability ? "Enter what you owe as a positive amount." : undefined}
            inputMode="decimal"
          />
          <TextField
            label="Balance as at"
            type="date"
            value={openingDate}
            onChange={(e) => setOpeningDate(e.currentTarget.value)}
            error={errors.opening_date}
          />
        </div>
        <TextField
          label="Notes"
          value={notes}
          onChange={(e) => setNotes(e.currentTarget.value)}
          error={errors.notes}
        />
      </form>
    </Modal>
  );
}
