// Account presentation helpers. Classification must match
// `AccountKind::is_liability` / `is_valued` in src-tauri/core/src/accounts.rs.
import type { Account, AccountKind } from "./api";

export const KIND_LABELS: Record<AccountKind, string> = {
  transaction: "Transaction",
  offset: "Offset",
  savings: "Savings",
  credit_card: "Credit card",
  home_loan: "Home loan",
  loan: "Loan",
  super: "Super",
  shares: "Shares",
  investment: "Investment",
  property: "Property",
  cash: "Cash",
  other: "Other",
};

export type AccountGroup = { id: string; label: string; kinds: AccountKind[] };

/** Accounts page sections, in order. Every kind appears exactly once. */
export const ACCOUNT_GROUPS: AccountGroup[] = [
  { id: "banking", label: "Banking", kinds: ["transaction", "offset", "savings", "cash"] },
  { id: "credit", label: "Credit & loans", kinds: ["credit_card", "home_loan", "loan"] },
  { id: "investments", label: "Investments & super", kinds: ["super", "shares", "investment", "property"] },
  { id: "other", label: "Other", kinds: ["other"] },
];

/** Kind picker order (matches Rust `AccountKind::ALL`). */
export const KIND_ORDER: AccountKind[] = ACCOUNT_GROUPS.flatMap((g) => g.kinds);

export function isLiability(kind: AccountKind): boolean {
  return kind === "credit_card" || kind === "home_loan" || kind === "loan";
}

export function isValued(kind: AccountKind): boolean {
  return kind === "super" || kind === "shares" || kind === "investment" || kind === "property";
}

export function canBeOffsetTarget(kind: AccountKind): boolean {
  return kind === "home_loan" || kind === "loan";
}

/** "063000" -> "063-000". Leaves anything unexpected untouched. */
export function formatBsb(bsb: string | null): string {
  if (!bsb) return "";
  return /^\d{6}$/.test(bsb) ? `${bsb.slice(0, 3)}-${bsb.slice(3)}` : bsb;
}

/** Group accounts into the page sections, dropping empty groups. Keeps the
 *  order the backend returned within each group. */
export function groupAccounts(accounts: Account[]): { group: AccountGroup; accounts: Account[] }[] {
  return ACCOUNT_GROUPS.map((group) => ({
    group,
    accounts: accounts.filter((a) => group.kinds.includes(a.kind)),
  })).filter((g) => g.accounts.length > 0);
}

/** Sum of balances in cents (integers stay exact up to 2^53). */
export function totalCents(accounts: Account[]): number {
  return accounts.reduce((sum, a) => sum + a.opening_balance, 0);
}
