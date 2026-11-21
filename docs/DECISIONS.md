# Decisions

Short, dated entries. Future sessions follow these without re-deciding.

- **2026-11-20 — Stack: Tauri 2 + Rust + React/TypeScript.** Standalone
  installable app with its own native window; no browser UI.
- **2026-11-20 — Platforms: Windows and Linux.** Installers: NSIS + MSI;
  AppImage, deb, rpm.
- **2026-11-20 — No auto-updater.** Repo stays private; new versions are
  installed manually from draft GitHub Releases.
- **2026-11-20 — Local-first storage in SQLite** in the OS app-data dir.
  No cloud sync.
- **2026-11-20 — Money as integer cents (`i64`)**, never floating point.
- **2026-11-20 — AUD only.** No multi-currency; no currency column needed
  beyond a fixed constant.
- **2026-11-20 — Password required at launch; whole database encrypted with
  SQLCipher.** The key is derived from the password. There is no recovery: a
  forgotten password means the data is unreadable (backups are encrypted too).
  Built into the DB layer from the start (M0), not bolted on later.
- **2026-11-20 — Import target: Bank of Melbourne** (Westpac group, same
  platform as St.George/BankSA). CSV export is the primary path. Personal
  online-banking exports are limited to a recent window (~3 months / 500
  transactions reported), so the owner should import regularly and duplicate
  detection must handle overlapping files.
- **2026-11-20 — Open banking (CDR) deferred.** CDR data can only flow to
  ACCC-accredited recipients (or their representatives); there is no
  machine-readable direct-to-consumer API. The only route is a paid cloud
  aggregator, which conflicts with local-first. Revisit if that changes.
- **2026-11-20 — Windows installers unsigned** for now (SmartScreen warning
  accepted; personal use).
- **2026-11-20 — Work is planned around usage limits:** small roadmap tasks,
  frequent pushes, HANDOFF.md updated at the end of every session. See AGENTS.md.
- **2026-11-20 — Logic lives in `plutus-core`** (`src-tauri/core`), a crate with
  no GUI dependencies; the Tauri crate is a thin shell. Keeps the core
  testable everywhere, including hosts without GTK/WebKit.
- **2026-11-20 — SQLCipher via `rusqlite` `bundled-sqlcipher-vendored-openssl`;
  key derivation is SQLCipher 4's default** (PBKDF2-HMAC-SHA512, 256k
  iterations, random per-file salt). Password passed as passphrase with
  `PRAGMA key`. Migrations are a small in-house runner on `PRAGMA user_version`
  (no extra crate).
- **2026-11-20 — Icons: `lucide-react`** (tree-shaken, only used icons ship).
- **2026-11-20 — No router library yet.** Page switching is a `PageId` state in
  `src/shell/AppShell.tsx`; pages are registered in `src/shell/pages.ts`. Add
  `react-router` only if deep links/history become necessary.
- **2026-11-20 — Money formatting split:** Rust parses and does all arithmetic;
  the UI only formats cents for display with `src/lib/money.ts`
  (`formatCents`), whose tests mirror the Rust `Display` tests. Change both
  together.
- **2026-11-20 — UI tests: Vitest + Testing Library (jsdom)**, run in CI via
  `npm test`. jsdom doesn't blur disabled inputs or do layout, so browser-only
  behaviour needs either a simulated step in the test or a check in the mocked
  browser preview.
- **2026-11-20 — Account kinds:** transaction, offset, savings, credit_card,
  home_loan, loan, super, shares, investment, property, cash, other. Owner's
  accounts are currently all offset accounts. Super/shares/investment/property
  are *valued* kinds (market value via valuations, not bank transactions).
- **2026-11-20 — Offset accounts may link to a home_loan/loan**
  (`offset_for`), several per loan; the link is optional so offsets work
  without the loan being tracked. A loan with linked offsets can't change to a
  non-loan kind.
- **2026-11-20 — Signed balances:** assets positive, liabilities negative
  (net worth = plain sum). The UI may accept "amount owed" as positive for
  liabilities and negate it, but storage is signed.
- **2026-11-20 — Accounts are archived, not deleted.** Active names are unique
  case-insensitively; archiving frees the name.
- **2026-11-20 — Dates use `chrono::NaiveDate`** (no clock features), stored
  as `YYYY-MM-DD` TEXT, serialised as the same string.
- **2026-11-20 — Validation errors** are `DbError::Invalid { field, message }`
  (code `invalid`) and `Conflict { field, message }` (code `conflict`); the UI
  shows `message` on the matching form field.
- **2026-11-20 — One money parser.** Typed amounts go through the
  `parse_money` command (Rust `Money::parse`); the UI never parses money.
  Errors come back as code `invalid_amount` with a friendly message.
- **2026-11-20 — Accounts page loads all accounts (archived included)** and
  filters client-side, so offset links to archived loans can still be named.
- **2026-11-20 — Balance = opening balance + transactions dated on/after the
  opening date** (owner's choice). Older transactions (imported history) are
  stored and listed but already reflected in the opening balance, so they're
  not counted again. Moving the opening date earlier brings them in.
- **2026-11-20 — Transaction amounts are signed from the account's view:**
  money in positive, out negative. At least one of payee / raw description is
  required. Zero amounts allowed.
- **2026-11-20 — Archived accounts accept no new transactions** (or
  transactions moved into them), but their existing history stays editable.
- **2026-11-20 — Columns for later features are added by migrations** with
  `ALTER TABLE ... ADD COLUMN` (nullable, FK allowed): `category_id` (M3),
  `transfer_id` (M2 transfers), `import_hash` (M4). Not pre-created.
- **2026-11-20 — Migration tests upgrade from every older schema version**
  (`upgrades_from_every_older_version`), since the owner's real database
  exists from v0.2.0 onward.
- **2026-11-21 — Transactions are entered as Money out / Money in plus a
  positive amount;** the sign comes from the choice (a typed minus is
  ignored). Display keeps signed amounts.
- **2026-11-21 — Dates display as "5 Nov 2026"** via `src/lib/dates.ts`,
  formatted from the `YYYY-MM-DD` string directly (never through `Date`
  parsing, which can shift the day across timezones).
- **2026-11-21 — Deleting a transaction needs a second click** ("Really
  delete?") in the edit dialog; no undo yet.
- **2026-11-21 — Running balances are computed in SQL** (window function over
  the account's whole history, then filtered/paged), so date filters and
  paging never change the figures. Shown only when the Transactions page is
  filtered to one account; pre-opening rows show "—".
- **2026-11-21 — Transfers are two linked transactions** sharing
  `transfer_id` (the outgoing leg's id), created/updated/deleted together
  in one SQL transaction. Payees are generated ("Transfer to/from X").
  Ordinary edits may change a leg's payee, memo and cleared flag only;
  account/date/amount change via `transfers::update`. Deleting either leg
  deletes both. An existing ordinary transaction can't be converted into a
  transfer (delete and re-add); that may come with import matching (M4).
- **2026-11-21 — Categories are a two-level tree** (groups → categories).
  Groups are `income` or `expense` (for budgets/reports); 11 groups and 47
  Australian-household defaults are seeded by migration 0005 and can be
  renamed/archived. Archive, never delete (like accounts). Active names
  unique per group (groups globally), case-insensitive. Transfers are not
  categorised. `transactions.category_id` added (nullable FK, RESTRICT).
- **2026-11-21 — Import before category assignment.** The owner supplied
  the Bank of Melbourne CSV format, so M4's importer comes next (it saves
  the most typing); M3 assignment/rules follow. BoM dates are MM/DD/YYYY;
  the importer parses that format explicitly, never guesses day/month order.
- **2026-11-21 — Import de-duplication by key, not fuzzy matching.**
  `import_key` = `date|cents|lowercased description|n` where n numbers
  identical rows within one file; stored on the transaction, unique per
  account (migration 0006). Re-importing overlaps skips them; committing
  twice is harmless. Rows matching a hand-entered transaction (same date +
  amount, no key) are `possible_match`; rows before the opening date are
  `before_opening`; both skipped unless ticked. Imported rows are cleared;
  payee is guessed from Eftpos descriptions, bank text kept as description.
  The file's balance chain is checked and the projected balance compared
  with the bank's. The UI reads the file as text (`<input type=file>`), so
  no dialog plugin is needed.
- **2026-11-21 — Category assignment rules.** Only active categories (in
  active groups) can be newly assigned; a transaction keeps an archived
  category it already has. Transfer legs never get a category (rejected on
  edit, skipped in bulk). The TS `TransactionInput.category_id` is required
  so every edit path (e.g. the Cleared toggle) sends it back instead of
  silently clearing it. Bulk assign is per page selection.
- **2026-11-21 — Rules are ordered "contains" matches.** Case-insensitive
  substring of payee, description or either (whitespace collapsed),
  optionally money in/out only; first match by `sort_order` wins. Rules
  fill only missing categories, skip transfers, and rules pointing at
  archived categories are ignored. Applied automatically on import (shown
  in the preview) and on demand ("Apply to uncategorised"). No regexes:
  simpler to explain, and enough for bank text. Rules can be deleted (they
  are settings, not history).
- **2026-11-21 — Budgets carry forward.** A budget amount set for a month
  applies to that month and every later month until changed (one row per
  change in `budgets`, migration 0008), so the owner sets it once. 0 stops
  budgeting from that month; clearing a month's change reverts to the
  earlier amount. Expense categories only. Spent = money out minus refunds
  in the calendar month; transfers never count. Income is reported as a
  total, not budgeted. Rollover: see below.
- **2026-11-21 — Rollover is a per-category option, off by default**
  (owner: "shouldn't carry over by default but make it an option").
  Migration 0009 adds `categories.budget_rollover`. With it on, each
  budgeted month's budget − spent is carried into the next month (negative
  when overspent); a month with no budget resets the carry. Month totals
  exclude carry-overs; the line's `remaining` includes them.
- **2026-11-21 — Order after budgets: dashboard & reports (M6) before
  valuations (M5b)** (owner's choice).
- **2026-11-21 — Report definitions.** Net worth = sum of all account
  balances (archived included); assets = positive balances, debts =
  negative. Month income/spending = money in/out excluding transfers
  (refunds count as income in these totals). Spending by category = net
  per expense category (refunds offset spending) plus uncategorised money
  out only (uncategorised money in isn't treated as a refund).
- **2026-11-21 — Charts are hand-rolled SVG** (`src/components/Charts.tsx`:
  `PairedBarChart`, `LineChart`), no chart library: smaller installer,
  matching style, and each chart carries a visually hidden data table for
  screen readers. The net-worth line scales to the data (not forced to
  include $0) so a mortgage doesn't flatten the trend. Net worth history
  counts each account from its opening date.
