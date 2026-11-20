// Typed wrappers around the Rust commands in src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";

export type AppInfo = {
  name: string;
  version: string;
  os: string;
  min_password_chars: number;
};

export type VaultStatus = "new" | "locked" | "unlocked";

/** Error shape every Rust command rejects with. `field` is set for
 *  validation errors (`invalid` / `conflict`) and names the form field. */
export type CommandError = { code: string; message: string; field?: string };

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
      default: {
        const m = e.message.charAt(0).toUpperCase() + e.message.slice(1);
        return /[.!?]$/.test(m) ? m : m + ".";
      }
    }
  }
  return String(e);
}

/** Split a rejected command into a per-field message (for form inputs) and
 *  a general message (for a toast). Exactly one of the two is set. */
export function splitError(e: unknown): { field: Record<string, string>; general: string | null } {
  if (isCommandError(e) && e.field && (e.code === "invalid" || e.code === "conflict")) {
    return { field: { [e.field]: e.message }, general: null };
  }
  return { field: {}, general: errorMessage(e) };
}

// ---- Accounts (mirror plutus_core::accounts) ----

export type AccountKind =
  | "transaction"
  | "offset"
  | "savings"
  | "credit_card"
  | "home_loan"
  | "loan"
  | "super"
  | "shares"
  | "investment"
  | "property"
  | "cash"
  | "other";

/** Money is integer cents (signed: liabilities negative); dates are YYYY-MM-DD. */
export type AccountInput = {
  name: string;
  kind: AccountKind;
  institution: string | null;
  bsb: string | null;
  account_number: string | null;
  opening_balance: number;
  opening_date: string;
  offset_for: number | null;
  notes: string | null;
};

export type Account = AccountInput & {
  id: number;
  archived_at: string | null;
  created_at: string;
};

/** Current balance per account id (cents). Keys are id strings (JSON). */
export type Balances = Record<string, number>;

// ---- Transactions (mirror plutus_core::transactions) ----

/** Amount is signed cents from the account's view: in positive, out negative. */
export type TransactionInput = {
  account_id: number;
  date: string;
  amount: number;
  payee: string | null;
  description: string | null;
  memo: string | null;
  cleared: boolean;
};

export type Transaction = TransactionInput & {
  id: number;
  created_at: string;
  updated_at: string | null;
};

/** A listed transaction with the account balance after it. `running_balance`
 *  is null unless the query is for one account, and for rows dated before the
 *  account's opening date. */
export type LedgerRow = Transaction & { running_balance: number | null };

/** Dates inclusive (YYYY-MM-DD). Newest first; page size defaults to 200, max 1000. */
export type TransactionQuery = {
  account_id?: number | null;
  from?: string | null;
  to?: string | null;
  limit?: number | null;
  offset?: number | null;
};

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  createVault: (password: string) => invoke<void>("create_vault", { password }),
  unlockVault: (password: string) => invoke<void>("unlock_vault", { password }),
  lockVault: () => invoke<void>("lock_vault"),
  // Tauri maps camelCase JS args to snake_case Rust params.
  changePassword: (current: string, newPassword: string) =>
    invoke<void>("change_password", { current, newPassword }),

  /** Strict money parse in Rust; resolves to cents, rejects with `invalid_amount`. */
  parseMoney: (text: string) => invoke<number>("parse_money", { text }),

  listAccounts: (includeArchived = false) =>
    invoke<Account[]>("list_accounts", { includeArchived }),
  createAccount: (input: AccountInput) => invoke<Account>("create_account", { input }),
  updateAccount: (id: number, input: AccountInput) =>
    invoke<Account>("update_account", { id, input }),
  setAccountArchived: (id: number, archived: boolean) =>
    invoke<Account>("set_account_archived", { id, archived }),
  accountBalances: () => invoke<Balances>("account_balances"),

  listTransactions: (query: TransactionQuery = {}) =>
    invoke<LedgerRow[]>("list_transactions", { query }),
  countTransactions: (query: TransactionQuery = {}) =>
    invoke<number>("count_transactions", { query }),
  addTransaction: (input: TransactionInput) => invoke<Transaction>("add_transaction", { input }),
  updateTransaction: (id: number, input: TransactionInput) =>
    invoke<Transaction>("update_transaction", { id, input }),
  deleteTransaction: (id: number) => invoke<void>("delete_transaction", { id }),
};
