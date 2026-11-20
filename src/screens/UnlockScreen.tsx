import { FormEvent, useEffect, useRef, useState } from "react";
import { Button, TextField } from "../components";
import { api, errorMessage } from "../lib/api";

type Props = { onUnlocked: () => void };

/** Every launch: unlock the encrypted database. */
export function UnlockScreen({ onUnlocked }: Props) {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!password || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.unlockVault(password);
      onUnlocked();
    } catch (err) {
      setError(errorMessage(err));
      setPassword("");
      setBusy(false);
    }
  }

  // After a failed attempt, return focus to the field once it is re-enabled.
  // (Focusing inside the catch block is too early: the input is still
  // disabled until React commits, and browsers ignore focus on disabled inputs.)
  useEffect(() => {
    if (error && !busy) input.current?.focus();
  }, [error, busy]);

  return (
    <main className="auth">
      <form className="card" onSubmit={submit}>
        <h1>Plutus</h1>
        <p className="muted">Enter your password to unlock your data.</p>

        <TextField
          ref={input}
          label="Password"
          type="password"
          autoFocus
          autoComplete="current-password"
          value={password}
          disabled={busy}
          onChange={(e) => setPassword(e.currentTarget.value)}
          error={error}
        />

        <Button type="submit" disabled={!password} busy={busy} busyLabel="Unlocking…">
          Unlock
        </Button>
      </form>
    </main>
  );
}
