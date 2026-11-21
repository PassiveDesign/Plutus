# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-20

## Current state

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
- **Transactions page done:** `src/pages/TransactionsPage.tsx` (account +
  inclusive date filters, 100/page paging with "1–100 of N", account column
  hidden when filtered, cleared checkbox toggles in place, empty/no-match/
  no-accounts states, load-error retry) and `TransactionForm.tsx` (Money
  out/in + positive amount via `parse_money`, defaults to the filtered
  account and today, archived accounts hidden except the transaction's own,
  inline field errors, two-step delete). `src/lib/dates.ts`. Inputs and
  selects share a 2.75rem height. 93 UI tests (12 page, 6 dates); checked in
  mocked preview, fixed the Direction radio layout there.
- **Running balance done:** `transactions::list_with_running_balance` →
  `LedgerRow { transaction (flattened), running_balance }`; the
  `list_transactions` command now returns `LedgerRow`s. TransactionsPage
  shows a Balance column when filtered to one account ("—" before the
  opening date). 70 core tests, 95 UI tests.
- **Transfers done:** migration 0004 (schema v4, `transfer_id`),
  `plutus-core::transfers` (`TransferInput`, `Transfer`, create/get/update/
  delete), `Transaction.transfer_id` + `transfer_account_id` (other leg's
  account, via a correlated subquery in the `columns!()` macro). Commands
  `create_transfer`, `update_transfer` (delete uses `delete_transaction`).
  TransactionForm: Money out / Money in / Transfer; editing either leg edits
  the transfer from the outgoing side; "Really delete both sides?". 77 core,
  7 shell, 100 UI tests. **M2 complete.**
- **v0.3.0 published** (owner smoke test passed).
- **Category tree done (core + commands):** migration 0005 (schema v5:
  `category_groups`, `categories`, `transactions.category_id`), module
  `plutus-core::categories` (`tree`, `get_group`, `get`, `create_group`,
  `update_group`, `set_group_archived`, `create`, `update`, `set_archived`),
  commands `list_categories`, `create_category_group`,
  `update_category_group`, `set_category_group_archived`,
  `create_category`, `update_category`, `set_category_archived`; TS types +
  wrappers in `api.ts`. No UI yet. 84 core tests.
- **BoM import core done:** migration 0006 (schema v6, `import_key`),
  `plutus-core::import` (`ParsedRow`, `ImportRow`, `RowStatus`,
  `Preview`, `preview`, `commit`, `keys`) and `import::bom::parse` /
  `guess_payee` (csv crate). Commands `preview_bom_import(accountId,
  text)`, `commit_import(accountId, rows)`; TS types in `api.ts`. 96 core
  tests.
- **Import screen done:** `src/pages/ImportPage.tsx` (sidebar "Import",
  `FileUp`), reads the file with `file.text()`, previews, ticks `new` rows,
  duplicates disabled, balance warnings, commit + "View in Transactions"
  (AppShell passes `initialAccountId` to TransactionsPage). 111 UI tests.
- **v0.4.0 published** (owner: real BoM import "All good").
- **Category assignment done:** `Transaction.category_id` /
  `TransactionInput.category_id`, `transactions::set_category` (bulk, skips
  transfers), command `set_transaction_category`; `src/pages/CategorySelect.tsx`
  (grouped picker + `categoryIndex`), category picker in TransactionForm,
  Category column, row selection + bulk bar on TransactionsPage (table now
  `table-layout: fixed`, payee/description cut to one line). 99 core, 115 UI.
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

**M3 — Categories page** (ROADMAP M3 item 2): new sidebar page
"Categories" (icon `Tags`, after Import) or a Settings section — prefer a
page. List groups (income first) with their categories; add group (name +
income/expense), add category to a group, rename, move category to another
group, archive/restore (show archived toggle). Commands already exist
(`list_categories`, `create_category_group`, `update_category_group`,
`set_category_group_archived`, `create_category`, `update_category`,
`set_category_archived`). Show a count of transactions per category if
cheap (would need a new `category_usage` command — optional).
Then **auto-categorisation rules** (M3 item 3), then v0.5.0.

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
