import { FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { Button, TextField, useToast } from "../components";
import { api, BackupFile, errorMessage, isCommandError } from "../lib/api";

type Props = {
  minChars: number;
  /** Called after a restore, so the shell reloads the pages with the new data. */
  onDataReplaced?: () => void;
};

export function SettingsPage({ minChars, onDataReplaced }: Props) {
  return (
    <div className="settings">
      <BackupSection onDataReplaced={onDataReplaced} />
      <ChangePasswordSection minChars={minChars} />
    </div>
  );
}

/** "1.2 MB" / "640 KB" — good enough for a backup list. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** "2026-11-21 09:30" from an RFC 3339 stamp, without parsing or time zones. */
export function formatStamp(iso: string): string {
  const m = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})/.exec(iso);
  return m ? `${m[1]} ${m[2]}` : iso;
}

function BackupSection({ onDataReplaced }: { onDataReplaced?: () => void }) {
  const toast = useToast();
  const [folder, setFolder] = useState("");
  const [files, setFiles] = useState<BackupFile[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [dir, list] = await Promise.all([api.backupFolder(), api.listBackups()]);
      setFolder(dir);
      setFiles(list);
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function backUp() {
    setBusy(true);
    try {
      const file = await api.backupNow();
      await load();
      toast.success(`Backed up ${formatBytes(file.bytes)} to ${file.name}.`);
    } catch (err) {
      toast.error(`Couldn't back up: ${errorMessage(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function restore(name: string) {
    setBusy(true);
    try {
      await api.restoreBackup(name);
      setConfirming(null);
      toast.success("Restored. Your data is back to that backup.");
      onDataReplaced?.();
    } catch (err) {
      toast.error(`Couldn't restore: ${errorMessage(err)}`);
      setConfirming(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-section" aria-labelledby="backup-heading">
      <h2 id="backup-heading">Backup</h2>
      <p className="muted">
        A backup is a copy of the encrypted database file. It stays encrypted, so you need your
        password to open it. The newest 10 copies are kept.
      </p>
      <div className="backup-actions">
        <Button onClick={backUp} disabled={busy} busy={busy} busyLabel="Backing up…">
          Back up now
        </Button>
        {folder && (
          <span className="muted backup-folder">
            Saved in <code>{folder}</code>
          </span>
        )}
      </div>

      {error && (
        <div className="error-block" role="alert">
          Couldn't load backups: {error}{" "}
          <Button variant="ghost" onClick={() => void load()}>
            Try again
          </Button>
        </div>
      )}

      {files === null ? (
        !error && <p className="muted">Loading backups…</p>
      ) : files.length === 0 ? (
        <p className="muted">No backups yet. The first one is a good idea before you import.</p>
      ) : (
        <ul className="backup-list" aria-label="Backups">
          {files.map((f) => (
            <li key={f.name} className="backup-row">
              <span className="backup-name">{f.name}</span>
              <span className="muted">
                {formatStamp(f.modified)} · {formatBytes(f.bytes)}
              </span>
              {confirming === f.name ? (
                <span className="backup-confirm">
                  <span>Replace all current data?</span>
                  <Button
                    variant="danger"
                    onClick={() => void restore(f.name)}
                    disabled={busy}
                    busy={busy}
                    busyLabel="Restoring…"
                  >
                    Restore
                  </Button>
                  <Button variant="ghost" onClick={() => setConfirming(null)} disabled={busy}>
                    Cancel
                  </Button>
                </span>
              ) : (
                <Button variant="ghost" onClick={() => setConfirming(f.name)} disabled={busy}>
                  Restore
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
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
