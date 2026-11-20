import { FormEvent, useState } from "react";
import { Button, TextField } from "../components";
import { api, errorMessage } from "../lib/api";

type Props = { minChars: number; onCreated: () => void };

/** First run: choose the password that encrypts the database. */
export function SetupScreen({ minChars, onCreated }: Props) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [understood, setUnderstood] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const length = [...password].length;
  const tooShort = length < minChars;
  const mismatch = confirm.length > 0 && password !== confirm;
  const canSubmit = !tooShort && password === confirm && understood && !busy;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setBusy(true);
    setError(null);
    try {
      await api.createVault(password);
      onCreated();
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  }

  const lengthHint = `At least ${minChars} characters${password ? ` (${length}/${minChars})` : ""}. A long passphrase is best.`;

  return (
    <main className="auth">
      <form className="card" onSubmit={submit}>
        <h1>Welcome to Plutus</h1>
        <p className="muted">
          Choose a password. It encrypts all of your financial data on this computer and is
          required every time Plutus opens.
        </p>

        <TextField
          label="Password"
          type="password"
          autoFocus
          autoComplete="new-password"
          value={password}
          onChange={(e) => setPassword(e.currentTarget.value)}
          hint={lengthHint}
          error={password && tooShort ? lengthHint : null}
        />

        <TextField
          label="Confirm password"
          type="password"
          autoComplete="new-password"
          value={confirm}
          onChange={(e) => setConfirm(e.currentTarget.value)}
          error={mismatch ? "Passwords don't match." : null}
        />

        <div className="warning">
          <strong>There is no password recovery.</strong> If you forget it, your data cannot be
          opened by anyone, including you. Store it in a password manager.
        </div>

        <label className="check">
          <input
            type="checkbox"
            checked={understood}
            onChange={(e) => setUnderstood(e.currentTarget.checked)}
          />
          I understand my password cannot be recovered.
        </label>

        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}

        <Button type="submit" disabled={!canSubmit} busy={busy} busyLabel="Creating encrypted database…">
          Create password
        </Button>
      </form>
    </main>
  );
}
