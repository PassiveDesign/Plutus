// Typed wrappers around the Rust commands in src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";

export type AppInfo = {
  name: string;
  version: string;
  os: string;
  min_password_chars: number;
};

export type VaultStatus = "new" | "locked" | "unlocked";

/** Error shape every Rust command rejects with. */
export type CommandError = { code: string; message: string };

export function isCommandError(e: unknown): e is CommandError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

/** A user-facing message for any rejected command. */
export function errorMessage(e: unknown): string {
  if (isCommandError(e)) {
    switch (e.code) {
      case "wrong_password":
        return "Incorrect password.";
      case "newer_schema":
        return "This database was created by a newer version of Plutus. Please update the app.";
      default:
        return e.message.charAt(0).toUpperCase() + e.message.slice(1) + ".";
    }
  }
  return String(e);
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  createVault: (password: string) => invoke<void>("create_vault", { password }),
  unlockVault: (password: string) => invoke<void>("unlock_vault", { password }),
  lockVault: () => invoke<void>("lock_vault"),
};
