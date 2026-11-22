import { FormEvent, useCallback, useEffect, useState } from "react";
import { Button, Modal, TextField, useToast } from "../components";
import { Account, api, errorMessage, splitError, Valuation } from "../lib/api";
import { formatDate, todayIso } from "../lib/dates";
import { formatCents, plainCents } from "../lib/money";

type Props = {
  account: Account;
  onClose: () => void;
  /** Called after any change so the page can reload balances. */
  onSaved: () => void;
};

/**
 * Valuation history for one valued account (super, shares, investment,
 * property), with a form to record a new market value or correct an existing
 * one. Re-recording a date replaces that day's figure.
 */
export function ValuationForm({ account, onClose, onSaved }: Props) {
  const toast = useToast();
  const [rows, setRows] = useState<Valuation[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [editing, setEditing] = useState<Valuation | null>(null);
  const [date, setDate] = useState(todayIso());
  const [valueText, setValueText] = useState(plainCents(account.opening_balance));
  const [note, setNote] = useState("");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null);

  const load = useCallback(async () => {
    try {
      const list = await api.listValuations(account.id);
      setRows(list);
      setLoadError(null);
      // Start the form from the latest figure, so "update value" is one edit.
      if (list.length > 0) {
        setValueText(plainCents(list[0].value));
      }
    } catch (e) {
      setLoadError(errorMessage(e));
    }
  }, [account.id]);

  useEffect(() => {
    void load();
  }, [load]);

  function startEdit(v: Valuation) {
    setEditing(v);
    setDate(v.date);
    setValueText(plainCents(v.value));
    setNote(v.note ?? "");
    setErrors({});
    setConfirmDelete(null);
  }

  function resetForm() {
    setEditing(null);
    setDate(todayIso());
    setValueText(rows && rows.length > 0 ? plainCents(rows[0].value) : plainCents(account.opening_balance));
    setNote("");
    setErrors({});
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    setBusy(true);
    setErrors({});
    try {
      const cents = await api.parseMoney(valueText);
      const input = { date, value: cents, note: note.trim() === "" ? null : note.trim() };
      if (editing) {
        await api.updateValuation(editing.id, input);
        toast.success(`Valuation for ${formatDate(date)} updated.`);
      } else {
        await api.addValuation(account.id, input);
        toast.success(`Valuation for ${formatDate(date)} saved.`);
      }
      resetForm();
      await load();
      onSaved();
    } catch (err) {
      const { field, general } = splitError(err);
      if (Object.keys(field).length > 0) {
        setErrors(field);
      } else {
        toast.error(general ?? errorMessage(err));
      }
    } finally {
      setBusy(false);
    }
  }

  async function remove(v: Valuation) {
    if (confirmDelete !== v.id) {
      setConfirmDelete(v.id);
      return;
    }
    setBusy(true);
    try {
      await api.deleteValuation(v.id);
      toast.success(`Valuation for ${formatDate(v.date)} deleted.`);
      setConfirmDelete(null);
      if (editing?.id === v.id) resetForm();
      await load();
      onSaved();
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open
      title={`Valuations — ${account.name}`}
      onClose={onClose}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            Done
          </Button>
          <Button type="submit" form="valuation-form" busy={busy} busyLabel="Saving…">
            {editing ? "Update valuation" : "Save valuation"}
          </Button>
        </>
      }
    >
      <form id="valuation-form" className="form" onSubmit={(e) => void onSubmit(e)}>
        <p className="muted">
          {account.name} is tracked by market value. Its balance is the latest valuation below.
        </p>
        <div className="form-row">
          <TextField
            label="Date"
            type="date"
            value={date}
            onChange={(e) => setDate(e.currentTarget.value)}
            error={errors.date}
            required
          />
          <TextField
            label="Value"
            inputMode="decimal"
            value={valueText}
            onChange={(e) => setValueText(e.currentTarget.value)}
            error={errors.value}
            hint="e.g. 125,000.00"
            required
          />
        </div>
        <TextField
          label="Note"
          value={note}
          onChange={(e) => setNote(e.currentTarget.value)}
          error={errors.note}
          hint="Optional — e.g. “annual statement”."
        />
        {editing && (
          <p className="field-hint">
            Editing the valuation from {formatDate(editing.date)}.{" "}
            <button type="button" className="link-button" onClick={resetForm}>
              Cancel edit
            </button>
          </p>
        )}
      </form>

      {loadError ? (
        <p className="field-error" role="alert">
          {loadError}
        </p>
      ) : rows === null ? (
        <p className="muted" aria-busy="true">
          Loading…
        </p>
      ) : rows.length === 0 ? (
        <p className="muted">No valuations yet. Record one above to set the balance.</p>
      ) : (
        <table className="valuation-table">
          <caption>Recorded valuations, newest first</caption>
          <thead>
            <tr>
              <th scope="col">Date</th>
              <th scope="col">Value</th>
              <th scope="col">Note</th>
              <th scope="col">
                <span className="sr-only">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((v) => (
              <tr key={v.id}>
                <td>{formatDate(v.date)}</td>
                <td className="money">{formatCents(v.value)}</td>
                <td className="muted">{v.note ?? ""}</td>
                <td className="row-actions">
                  <Button variant="ghost" onClick={() => startEdit(v)}>
                    Edit
                  </Button>
                  <Button
                    variant={confirmDelete === v.id ? "danger" : "ghost"}
                    onClick={() => void remove(v)}
                    busy={busy && confirmDelete === v.id}
                  >
                    {confirmDelete === v.id ? "Really delete?" : "Delete"}
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Modal>
  );
}
