# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-20

## Current state

- **Design language adopted (branch `design/passive-ui`, PR open):** Plutus
  now looks like PassiveSync via **Passive UI**, vendored in `src/kit/`
  (tokens, IBM Plex Sans + JetBrains Mono, shared components, timeline
  module). Gold accent; light/dark follows the system (`src/lib/theme.ts`).
  Shell is a top bar with tabs + Lock (`src/shell/TopBar.tsx`) and a status
  bar (`StatusBar.tsx`); `Sidebar.tsx` is gone. `App.css` rewritten on the
  tokens (678 → ~200 lines); component class names unchanged, so all tests
  stand (74 now, +2 for the theme). Negative amounts are no longer red (see
  DECISIONS). Checked in a mocked browser preview in both modes. No Rust or
  data changes. The account timeline waits for `list_transactions` (roadmap
  D, second item).
- Scaffold + CI green on Windows/Linux.
- **v0.1.0 tagged; Release workflow succeeded on first run** (run 44712658224).
  Draft release assets: `Plutus_0.1.0_x64-setup.exe` (3.2 MB),
  `Plutus_0.1.0_x64_en-US.msi` (4.4 MB), `Plutus_0.1.0_amd64.AppImage`
  (79.5 MB), `.deb` (4.8 MB), `.rpm` (4.8 MB). **Owner smoke test passed on
  2026-11-20** (setup validation, Ctrl+L, wrong/right unlock, change password,
  restart). UI reported snappy. Release published (no longer draft).
  **M0 complete.**
- **M1 accounts core done:** migration 0002 `accounts` (schema v2; v1 → v2
  upgrade tested), `plutus-core::accounts` (`AccountKind` with
  `is_liability`/`is_valued`/`can_be_offset_target`, `AccountInput`,
  `Account`, `get`/`list`/`create`/`update`/`set_archived`/`offsets_for`),
  validation + normalisation (BSB → 6 digits, account no. digits only),
  offset-link rules, case-insensitive unique active names.
- **Accounts plumbing done:** commands `list_accounts`, `create_account`,
  `update_account`, `set_account_archived` via `with_db` helper (shell tests
  cover locked → `locked`, create, duplicate → `conflict`/`name`).
  `CommandError.field` for validation errors. TS: `src/lib/api.ts` account
  types + wrappers + `splitError()`; `src/lib/accounts.ts` labels, groups,
  `isLiability`/`isValued`/`canBeOffsetTarget`, `formatBsb`, `groupAccounts`,
  `totalCents`. Kind display order pinned on both sides (Cash moved next to
  Savings).
- **Accounts page done:** `src/pages/AccountsPage.tsx` (grouped tables, group
  + net totals, offset subtitles, archived toggle/badge, empty state, load
  error retry) and `AccountForm.tsx` (modal; liabilities entered as positive
  "amount owed"; amounts via `parse_money`; inline field errors; archive/
  restore; keeps existing `offset_for`). `SelectField` component,
  `DataTable.rowClassName`. 10 page tests; checked in mocked preview
  (fixed a grid overflow in the form).
- **Offset linking UI done:** "Offsets loan" picker in `AccountForm` (active
  loans + current archived link; hint when no loans), loan rows show
  "N offsets · Net owed $X" / "Fully offset" via `loanSummary()`;
  `offsetTargets()` helper. 72 UI tests. **M1 complete.**
- **v0.2.0 published.** Owner smoke test passed: v0.1.0 → v0.2.0 install,
  v1 → v2 database upgrade on unlock, real accounts entered (home loan +
  linked offsets), restart persistence. **Owner has real data from here on
  — every migration must upgrade cleanly; never edit a shipped migration.**
- **M2 transactions core done:** migration 0003 (schema v3),
  `plutus-core::transactions` (`TransactionInput`, `Transaction`,
  `TransactionQuery`; `add`/`get`/`update`/`delete`/`list`/`count`),
  `accounts::balance`/`balances` (opening + txns on/after opening date).
  66 core tests incl. upgrade from every older schema.
- **Transaction commands + real balances done:** shell commands
  `account_balances`, `list_transactions`, `count_transactions`,
  `add/update/delete_transaction` (all via `with_db`; shell test covers
  round trip, balances JSON keyed by id string, field error, not found).
  TS types/wrappers in `src/lib/api.ts` (`Balances`, `Transaction*`).
  `accounts.ts` helpers take an optional `balanceOf` (`balanceLookup()`
  falls back to opening balance); AccountsPage shows current balances in
  rows, group totals, net total and loan net owed; the form still edits the
  opening balance. 75 UI tests.
- **Open PR #1 (`design/passive-ui`, another session):** Passive UI redesign.
  Owner said leave it; merge conflicts expected in `AccountsPage.tsx`,
  `App.css`, docs. Note its commit `9713101` accidentally also contains this
  task's changes (swept in from the shared checkout) — identical code, so
  the merge should resolve to main's version.
- **UI shell done:** `src/shell/` — `pages.ts` (page registry: id, label,
  icon, summary, milestone), `Sidebar.tsx` (nav + Settings/Lock footer +
  version), `AppShell.tsx` (page state, Ctrl+L locks). Every page is
  `src/pages/PlaceholderPage.tsx` for now. To build a real page: create
  `src/pages/<Name>Page.tsx` and switch on `current` in `AppShell`.
  Light + dark themes via CSS variables in `App.css`. Checked in the mocked
  browser preview (nav, active state, dark mode, Ctrl+L, Lock button).
- **Change password done:** `Vault::change_password(current, new)` verifies
  the current password on a separate connection (`Database::verify_password`),
  enforces policy, then rekeys. Command `change_password` (JS arg
  `newPassword`). `src/pages/SettingsPage.tsx` is the first real page
  (AppShell switches on `current === "settings"`). 40 core + 43 UI tests;
  flow checked in mocked browser preview.
- **Shared UI basics done:** `src/components/` (Button variants + busy,
  TextField with hint/error/a11y, Modal with focus trap/Escape/backdrop,
  DataTable with money columns + empty state + keyboard rows, Toast provider
  — errors sticky, others 4s). `src/lib/money.ts` `formatCents`/`plainCents`.
  Vitest set up (39 tests: components, money, password screens with mocked
  API); CI runs `npm test`. Setup/Unlock screens now use the components;
  fixed unlock-field refocus after a wrong password (verified in Chromium).
- **Money type done** (`plutus-core::money::Money`): strict parser (`$1,234.56`,
  `(12.30)`, `-$5`, rejects `1.234`/bad grouping), Display `$1,234.56`,
  plain `1234.56`, checked arithmetic capped at ±2^53−1 cents, serde as
  integer cents. 13 tests.
- **Encrypted DB layer** (`plutus-core::db`): SQLCipher, migrations, 0001 `meta`.
- **Password flow done:**
  - `plutus-core::vault::Vault` state machine (new/locked/unlocked), min 8
    chars for new passwords, stable error codes (`DbError::code()`).
  - Tauri commands in `src-tauri/src/lib.rs`: `vault_status`, `create_vault`,
    `unlock_vault`, `lock_vault` (create/unlock run in `spawn_blocking`).
    Errors reach the UI as `{ code, message }`.
  - UI: `src/lib/api.ts` (typed wrappers), `src/screens/SetupScreen.tsx`
    (confirm field, no-recovery warning + checkbox), `UnlockScreen.tsx`.
    `App.tsx` routes on vault status.
  - Core tests pass locally; UI flow checked in a browser against a mocked
    backend (create → lock → wrong pw error → unlock). **Not yet run as a real
    desktop app** — needs the GTK/WebKit packages or the v0.1.0 installers.

## Next task

**M2 — Transactions page** (`src/pages/TransactionsPage.tsx`, switch in
`AppShell`). Work in a worktree (see AGENTS.md).
- Filters: account (`SelectField`, "All accounts"), from/to dates.
- `DataTable`: Date, Payee (+ raw description muted), Account (name; hide
  when filtered to one), Amount (money column), Cleared (checkbox toggle
  calling `updateTransaction`). Newest first.
- Paging: 100/page with `countTransactions`; Prev/Next + "41–60 of 312".
- Add/Edit modal: account, date (default today), amount entered as
  money-in/money-out (two radio buttons + positive amount → signed cents via
  `parseMoney`), payee, description, memo, cleared. Delete with confirm.
  Field errors via `splitError`.
- Empty state; archived accounts excluded from the add picker.
- Tests with mocked api + mocked browser preview. If PR #1 has merged by
  then, follow its Passive UI rules (tokens, no raw colours).

## Open questions for the owner

- Bank of Melbourne CSV format: need the header row and one example of the
  date/amount format (made-up values are fine) before the M4 importer.
- Does the export menu also offer OFX/QFX/QIF?

## Environment notes

- Dev host lacks GTK/WebKit `-dev` packages (owner will install later), so
  only `plutus-core` compiles locally (`cargo test -p plutus-core`); the
  Tauri shell compiles in CI.
- Cold `plutus-core` build takes ~80s (SQLCipher + vendored OpenSSL).
- `gh` lives at `~/.local/bin/gh`; Rust via rustup in `~/.cargo`.

## Half-done work

None.
