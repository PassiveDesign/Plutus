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
  if (e instanceof Error) return e.message;
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

/** Current balance per account id (cents). Keys are id strings (JSON).
 *  Valued accounts (super, shares, investments, property) report their
 *  latest valuation; everything else opening balance + transactions. */
export type Balances = Record<string, number>;

// ---- Valuations (mirror plutus_core::valuations) ----

/** One market valuation. `value` is signed cents, assets positive. */
export type ValuationInput = {
  date: string;
  value: number;
  note: string | null;
};

export type Valuation = ValuationInput & {
  id: number;
  account_id: number;
  created_at: string;
};

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

// ---- Generic CSV imports (mirror plutus_core::import::generic) ----

export type DateFormat = "mdy" | "dmy" | "ymd";

/** Which column holds what. Column names match the header row, ignoring case
 *  and spaces. Amounts come from one signed column (`amount`) or a debit /
 *  credit pair; `payee` and `balance` are optional. */
export type ImportMapping = {
  date: string;
  date_format: DateFormat;
  description: string;
  payee: string | null;
  amount: string | null;
  debit: string | null;
  credit: string | null;
  balance: string | null;
};

export type ImportFileShape = { headers: string[]; mapping: ImportMapping };

export type ImportPreset = { id: number; name: string; mapping: ImportMapping };

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

/** A calendar month, "YYYY-MM". */
export type Month = string;

export type BudgetLine = {
  category_id: number;
  group_id: number;
  /** Cents; null when no budget applies this month. */
  budgeted: number | null;
  /** Money out minus refunds (cents). */
  spent: number;
  /** budgeted − spent; negative = over budget. */
  remaining: number | null;
  /** Month the amount was set in (it carries forward until changed). */
  set_in: Month | null;
  /** Unspent (or overspent, negative) budget brought in; 0 unless rollover. */
  carried: number;
  rollover: boolean;
};

export type MonthBudget = {
  month: Month;
  lines: BudgetLine[];
  total_budgeted: number;
  total_spent_budgeted: number;
  total_remaining: number;
  unbudgeted_spent: number;
  uncategorised_spent: number;
  income: number;
};

export type ExportedFile = {
  name: string;
  path: string;
  bytes: number;
  /** How many transactions went into the file. */
  rows: number;
};

export type BackupFile = {
  name: string;
  path: string;
  bytes: number;
  /** RFC 3339, local time. */
  modified: string;
};

export type Dashboard = {
  month: Month;
  /** Cents. `debts` is negative. */
  net_worth: { assets: number; debts: number; net: number };
  /** This month, transfers excluded. `spending` is positive. */
  totals: { income: number; spending: number; net: number };
  /** Largest first, at most 5; category_id null = uncategorised. */
  top_spending: { category_id: number | null; spent: number }[];
  budgeted: number;
  budget_spent: number;
  budget_remaining: number;
  recent: Transaction[];
};

export type MonthRow = {
  month: Month;
  income: number;
  spending: number;
  net: number;
  /** Net worth at month end. */
  net_worth: number;
};

export type Report = {
  from: Month;
  to: Month;
  months: MonthRow[];
  spending_by_category: { category_id: number | null; spent: number }[];
  totals: { income: number; spending: number; net: number };
};

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

  /** Valuations for one valued account, newest first. */
  listValuations: (accountId: number) =>
    invoke<Valuation[]>("list_valuations", { accountId }),
  /** Re-recording a date replaces that day's valuation. */
  addValuation: (accountId: number, input: ValuationInput) =>
    invoke<Valuation>("add_valuation", { accountId, input }),
  updateValuation: (id: number, input: ValuationInput) =>
    invoke<Valuation>("update_valuation", { id, input }),
  deleteValuation: (id: number) => invoke<void>("delete_valuation", { id }),

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
  /** Header row and a guessed mapping for any other bank's CSV. */
  importShape: (text: string) => invoke<ImportFileShape>("import_shape", { text }),
  previewGenericImport: (accountId: number, text: string, mapping: ImportMapping) =>
    invoke<ImportPreview>("preview_generic_import", { accountId, text, mapping }),
  listImportPresets: () => invoke<ImportPreset[]>("list_import_presets"),
  /** Saving an existing name replaces its mapping. */
  saveImportPreset: (name: string, mapping: ImportMapping) =>
    invoke<ImportPreset>("save_import_preset", { name, mapping }),
  deleteImportPreset: (id: number) => invoke<void>("delete_import_preset", { id }),
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
  budgetMonth: (month: Month) => invoke<MonthBudget>("budget_month", { month }),
  /** Applies from `month` onwards until changed; 0 stops budgeting. */
  setBudget: (categoryId: number, month: Month, amount: number) =>
    invoke<void>("set_budget", { categoryId, month, amount }),
  setBudgetRollover: (categoryId: number, on: boolean) =>
    invoke<void>("set_budget_rollover", { categoryId, on }),
  /** Undo the change made in exactly `month`. */
  clearBudget: (categoryId: number, month: Month) => invoke<void>("clear_budget", { categoryId, month }),
  dashboard: (month: Month) => invoke<Dashboard>("dashboard", { month }),
  /** Both months inclusive; at most 10 years. */
  report: (from: Month, to: Month) => invoke<Report>("report", { from, to }),
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
  /** Copy the encrypted database into the backups folder. */
  exportTransactions: (query: TransactionQuery) =>
    invoke<ExportedFile>("export_transactions", { query }),
  backupNow: () => invoke<BackupFile>("backup_now"),
  listBackups: () => invoke<BackupFile[]>("list_backups"),
  /** Folder holding the copies, for display. */
  backupFolder: () => invoke<string>("backup_folder"),
  /** Replace all data with the named copy. The app stays unlocked. */
  restoreBackup: (name: string) => invoke<void>("restore_backup", { name }),
  updateTransfer: (id: number, input: TransferInput) =>
    invoke<Transfer>("update_transfer", { id, input }),
};
