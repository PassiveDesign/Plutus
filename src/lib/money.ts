// Money display on the UI side.
//
// Amounts arrive from Rust as integer cents (see `plutus_core::Money`). Rust
// is authoritative for *parsing* user input and for all arithmetic; the UI
// only *formats* for display, and must match Rust's `Display` exactly:
//   $1,234.56   -$12.30   $0.00

/** Largest magnitude in cents that Rust will ever send (2^53 − 1). */
export const MAX_CENTS = Number.MAX_SAFE_INTEGER;

function assertCents(cents: number): void {
  if (!Number.isSafeInteger(cents)) {
    throw new RangeError(`money must be a safe integer number of cents, got ${cents}`);
  }
}

/** `$1,234.56` / `-$12.30` — matches Rust `Money`'s Display. */
export function formatCents(cents: number): string {
  assertCents(cents);
  const negative = cents < 0;
  const abs = Math.abs(cents);
  const dollars = Math.floor(abs / 100);
  const rem = abs % 100;
  const grouped = dollars.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return `${negative ? "-" : ""}$${grouped}.${rem.toString().padStart(2, "0")}`;
}

/** `1234.56` / `-12.30` — plain form for edit fields, matches `to_plain_string`. */
export function plainCents(cents: number): string {
  assertCents(cents);
  const negative = cents < 0;
  const abs = Math.abs(cents);
  return `${negative ? "-" : ""}${Math.floor(abs / 100)}.${(abs % 100).toString().padStart(2, "0")}`;
}
