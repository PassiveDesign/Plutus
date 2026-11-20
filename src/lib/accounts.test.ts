import { describe, expect, it } from "vitest";
import type { Account, AccountKind } from "./api";
import {
  ACCOUNT_GROUPS,
  canBeOffsetTarget,
  formatBsb,
  groupAccounts,
  isLiability,
  isValued,
  KIND_LABELS,
  KIND_ORDER,
  totalCents,
} from "./accounts";

const ALL_KINDS = Object.keys(KIND_LABELS) as AccountKind[];

function acct(id: number, kind: AccountKind, cents = 0): Account {
  return {
    id,
    name: `A${id}`,
    kind,
    institution: null,
    bsb: null,
    account_number: null,
    opening_balance: cents,
    opening_date: "2026-11-01",
    offset_for: null,
    notes: null,
    archived_at: null,
    created_at: "2026-11-01T00:00:00Z",
  };
}

describe("account kinds", () => {
  it("every kind is in exactly one group, in Rust's display order", () => {
    expect([...KIND_ORDER].sort()).toEqual([...ALL_KINDS].sort());
    expect(new Set(KIND_ORDER).size).toBe(KIND_ORDER.length);
    // Must equal AccountKind::ALL in src-tauri/core/src/accounts.rs.
    expect(KIND_ORDER).toEqual([
      "transaction", "offset", "savings", "cash",
      "credit_card", "home_loan", "loan",
      "super", "shares", "investment", "property",
      "other",
    ]);
  });

  it("classification matches the Rust side", () => {
    expect(ALL_KINDS.filter(isLiability).sort()).toEqual(["credit_card", "home_loan", "loan"]);
    expect(ALL_KINDS.filter(isValued).sort()).toEqual(["investment", "property", "shares", "super"]);
    expect(ALL_KINDS.filter(canBeOffsetTarget).sort()).toEqual(["home_loan", "loan"]);
  });
});

describe("formatBsb", () => {
  it.each([
    ["063000", "063-000"],
    [null, ""],
    ["", ""],
    ["12345", "12345"],
  ])("%s -> %s", (input, out) => expect(formatBsb(input)).toBe(out));
});

describe("groupAccounts / totalCents", () => {
  it("groups in section order, drops empty sections, keeps backend order", () => {
    const accounts = [acct(1, "offset", 500), acct(2, "home_loan", -45_000_000), acct(3, "offset", 250)];
    const groups = groupAccounts(accounts);
    expect(groups.map((g) => g.group.id)).toEqual(["banking", "credit"]);
    expect(groups[0].accounts.map((a) => a.id)).toEqual([1, 3]);
    expect(totalCents(groups[0].accounts)).toBe(750);
    expect(totalCents(accounts)).toBe(-44_999_250);
  });

  it("group ids are unique", () => {
    expect(new Set(ACCOUNT_GROUPS.map((g) => g.id)).size).toBe(ACCOUNT_GROUPS.length);
  });
});
