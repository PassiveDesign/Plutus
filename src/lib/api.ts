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
  /** Always null on transfer legs. Send the current value back on edits,
   *  or the category is cleared. */
  category_id: number | null;
};

export type Transaction = TransactionInput & {
  id: number;
  created_at: string;
  updated_at: string | null;
  /** Shared by both legs of a transfer; null for ordinary transactions. */
  transfer_id: number | null;
  /** For transfer legs: the account on the other side. */
  transfer_account_id: number | null;
};

/** Amount is positive cents moved from `from_account` to `to_account`. */
export type TransferInput = {
  from_account: number;
  to_account: number;
  date: string;
  amount: number;
  memo: string | null;
};

export type Transfer = { id: number; from: Transaction; to: Transaction };

export type ImportRow = {
  /** 1-based line in the file. */
  line: number;
  date: string;
  /** Signed cents, money in positive. */
  amount: number;
  description: string;
  payee: string | null;
  /** Bank's balance after this row (cents). */
  balance: number | null;
  import_key: string;
};

/** new = imported by default; duplicate = already imported (always skipped);
 *  possible_match / before_opening = skipped unless the owner ticks them. */
export type ImportStatus = "new" | "duplicate" | "possible_match" | "before_opening";

export type ImportPreviewRow = ImportRow & {
  status: ImportStatus;
  matched_transaction_id: number | null;
  /** Category a rule will give the row on import. */
  category_id: number | null;
};

export type ImportPreview = {
  account_id: number;
  rows: ImportPreviewRow[];
  order: "newest_first" | "oldest_first" | "unknown";
  /** Lines whose balance doesn't follow from the previous row's. */
  balance_breaks: number[];
  bank_balance: number | null;
  current_balance: number;
  /** Plutus balance if the `new` rows are imported. */
  balance_after: number;
};

export type ImportResult = { imported: number; skipped: number };

export type MatchField = "payee" | "description" | "either";
export type RuleDirection = "in" | "out";

/** "If <match_field> contains <pattern> (and money goes <direction>), use
 *  <category_id>." Tried in order; first match wins; never overwrites. */
export type RuleInput = {
  pattern: string;
  match_field: MatchField;
  direction: RuleDirection | null;
  category_id: number;
};

export type Rule = RuleInput & { id: number; sort_order: number };

export type GroupKind = "income" | "expense";

export type Category = {
  id: number;
  group_id: number;
  name: string;
  sort_order: number;
  archived_at: string | null;
};

/** Income groups come first in the tree, then expense groups. */
export type CategoryGroup = {
  id: number;
  name: string;
  kind: GroupKind;
  sort_order: number;
  archived_at: string | null;
  categories: Category[];
};

export type GroupInput = { name: string; kind: GroupKind };
export type CategoryInput = { group_id: number; name: string };

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
  /** Deleting either leg of a transfer deletes both. */
  deleteTransaction: (id: number) => invoke<void>("delete_transaction", { id }),
  previewBomImport: (accountId: number, text: string) =>
    invoke<ImportPreview>("preview_bom_import", { accountId, text }),
  /** Send back the preview rows to import (as received, minus `status`). */
  commitImport: (accountId: number, rows: ImportRow[]) =>
    invoke<ImportResult>("commit_import", { accountId, rows }),
  listRules: () => invoke<Rule[]>("list_rules"),
  createRule: (input: RuleInput) => invoke<Rule>("create_rule", { input }),
  updateRule: (id: number, input: RuleInput) => invoke<Rule>("update_rule", { id, input }),
  deleteRule: (id: number) => invoke<void>("delete_rule", { id }),
  moveRule: (id: number, up: boolean) => invoke<Rule[]>("move_rule", { id, up }),
  /** Categorise uncategorised transactions; resolves to how many changed. */
  applyRules: () => invoke<number>("apply_rules"),
  listCategories: (includeArchived = false) =>
    invoke<CategoryGroup[]>("list_categories", { includeArchived }),
  /** Transactions per category id; unused categories are absent. */
  categoryUsage: () => invoke<Record<string, number>>("category_usage"),
  createCategoryGroup: (input: GroupInput) =>
    invoke<CategoryGroup>("create_category_group", { input }),
  updateCategoryGroup: (id: number, input: GroupInput) =>
    invoke<CategoryGroup>("update_category_group", { id, input }),
  setCategoryGroupArchived: (id: number, archived: boolean) =>
    invoke<CategoryGroup>("set_category_group_archived", { id, archived }),
  createCategory: (input: CategoryInput) => invoke<Category>("create_category", { input }),
  updateCategory: (id: number, input: CategoryInput) =>
    invoke<Category>("update_category", { id, input }),
  setCategoryArchived: (id: number, archived: boolean) =>
    invoke<Category>("set_category_archived", { id, archived }),
  /** Transfer legs are skipped; resolves to how many changed. */
  setTransactionCategory: (ids: number[], categoryId: number | null) =>
    invoke<number>("set_transaction_category", { ids, categoryId }),
  createTransfer: (input: TransferInput) => invoke<Transfer>("create_transfer", { input }),
  updateTransfer: (id: number, input: TransferInput) =>
    invoke<Transfer>("update_transfer", { id, input }),
};
