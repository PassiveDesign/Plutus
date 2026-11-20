import { FormEvent, useEffect, useRef, useState } from "react";
import { Button, TextField, useToast } from "../components";
import { api, errorMessage, isCommandError } from "../lib/api";

type Props = { minChars: number };

export function SettingsPage({ minChars }: Props) {
  return (
    <div className="settings">
      <ChangePasswordSection minChars={minChars} />
    </div>
  );
}

function ChangePasswordSection({ minChars }: { minChars: number }) {
  const toast = useToast();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [currentError, setCurrentError] = useState<string | null>(null);
  const currentRef = useRef<HTMLInputElement>(null);

  const nextLen = [...next].length;
  const tooShort = next.length > 0 && nextLen < minChars;
  const same = next.length > 0 && next === current;
  const mismatch = confirm.length > 0 && next !== confirm;
  const canSubmit =
    current.length > 0 && nextLen >= minChars && !same && next === confirm && !busy;

  // Refocus "current" after a wrong-password failure, once it is re-enabled.
  useEffect(() => {
    if (currentError && !busy) currentRef.current?.focus();
  }, [currentError, busy]);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setBusy(true);
    setCurrentError(null);
    try {
      await api.changePassword(current, next);
      setCurrent("");
      setNext("");
      setConfirm("");
      toast.success("Password changed. Use the new password next time you unlock Plutus.");
    } catch (err) {
      if (isCommandError(err) && err.code === "wrong_password") {
        setCurrentError("Current password is incorrect.");
        setCurrent("");
      } else {
        toast.error(`Couldn't change password: ${errorMessage(err)}`);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-section" aria-labelledby="pw-heading">
      <h2 id="pw-heading">Change password</h2>
      <p className="muted">
        Your password encrypts all Plutus data on this computer. Changing it re-encrypts the
        database; it only takes a moment.
      </p>
      <form className="settings-form" onSubmit={submit}>
        <TextField
          ref={currentRef}
          label="Current password"
          type="password"
          autoComplete="current-password"
          value={current}
          disabled={busy}
          onChange={(e) => setCurrent(e.currentTarget.value)}
          error={currentError}
        />
        <TextField
          label="New password"
          type="password"
          autoComplete="new-password"
          value={next}
          disabled={busy}
          onChange={(e) => setNext(e.currentTarget.value)}
          hint={`At least ${minChars} characters.`}
          error={
            tooShort
              ? `At least ${minChars} characters (${nextLen}/${minChars}).`
              : same
                ? "New password must be different from the current one."
                : null
          }
        />
        <TextField
          label="Confirm new password"
          type="password"
          autoComplete="new-password"
          value={confirm}
          disabled={busy}
          onChange={(e) => setConfirm(e.currentTarget.value)}
          error={mismatch ? "Passwords don't match." : null}
        />
        <div className="warning">
          <strong>There is still no recovery.</strong> Save the new password in your password
          manager before you continue.
        </div>
        <div>
          <Button type="submit" disabled={!canSubmit} busy={busy} busyLabel="Re-encrypting…">
            Change password
          </Button>
        </div>
      </form>
    </section>
  );
}
