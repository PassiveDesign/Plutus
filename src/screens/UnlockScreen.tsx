import { FormEvent, useRef, useState } from "react";
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
      input.current?.focus();
    }
  }

  return (
    <main className="auth">
      <form className="card" onSubmit={submit}>
        <h1>Plutus</h1>
        <p className="muted">Enter your password to unlock your data.</p>

        <label>
          Password
          <input
            ref={input}
            type="password"
            autoFocus
            autoComplete="current-password"
            value={password}
            disabled={busy}
            onChange={(e) => setPassword(e.currentTarget.value)}
          />
        </label>

        {error && <p className="error">{error}</p>}

        <button type="submit" disabled={!password || busy}>
          {busy ? "Unlocking…" : "Unlock"}
        </button>
      </form>
    </main>
  );
}
