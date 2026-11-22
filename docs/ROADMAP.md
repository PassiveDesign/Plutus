# Roadmap

Each `- [ ]` is one task, sized for a single short session. Work top to bottom
unless HANDOFF says otherwise. Tick tasks as they land on `main` with CI green.
Feature scope is a first draft — edit freely.

## M0 — Foundation

- [x] Repo, Tauri 2 + React/TS scaffold, CI + release workflows
- [x] Planning docs (AGENTS, ROADMAP, HANDOFF, DECISIONS)
- [x] Confirm first CI run green on Windows + Linux
- [x] Encrypted SQLite layer: `rusqlite` with bundled SQLCipher, open DB in app-data dir with a key derived from the password, migration runner, first migration (`meta` table), unit tests (wrong password rejected, migrations idempotent)
- [x] Password flow: first-run "create password" screen (with no-recovery warning), unlock screen on every launch, lock on close; DB stays closed until unlocked
- [x] Money type (`i64` cents, AUD only) with parse/format helpers (`$1,234.56`, negatives) and tests
- [x] UI shell: sidebar navigation (Dashboard, Accounts, Transactions, Budgets, Reports, Settings) with empty pages
- [x] Shared UI basics: layout, theme (light/dark), form + table components, error toast
- [x] Change-password in Settings (needs the UI shell; verify current password first) (SQLCipher rekey)
- [x] Cut `v0.1.0` release; install on Windows and Linux to prove the pipeline

## M1 — Accounts

- [x] Schema + Rust model + repository: accounts (12 kinds incl. offset, home loan, super, shares, investment, property), offset → loan links, BSB/account number, opening balance/date, archive; validation; tests
- [x] Tauri commands (`list_accounts`, `create_account`, `update_account`, `set_account_archived`) + typed TS API + field-error display (`invalid`/`conflict` codes → TextField errors)
- [x] Accounts page: grouped table (Banking / Credit & loans / Investments & super / Other) with balances, add/edit modal, archive/restore, show-archived toggle
- [x] Offset linking in the UI: pick a loan for an offset account; loan row shows its offsets and the net (loan + offsets) owed

## M1.5 — Release

- [x] Cut `v0.2.0` (accounts); owner enters real accounts; verify v0.1.0 → v0.2.0 database upgrade

## M2 — Transactions

- [x] Schema + model: transactions (account, date, signed amount, payee, raw description, memo, cleared); add/update/delete/list (filter by account + date range, paged) + count; account balances (opening + transactions from opening date); tests
- [x] Commands + typed TS API (`list_transactions`, `count_transactions`, `add/update/delete_transaction`, `account_balances`); Accounts page shows real balances
- [x] Transactions page: table (date, payee/description, account, amount, cleared), account + date filters, paging, add/edit/delete dialog
- [x] Running balance per account in the transactions table (needs account filter)
- [x] Transfers between accounts (paired transactions; migration adds `transfer_id`)

## M2.5 — Release

- [x] Cut `v0.3.0` (transactions, running balances, transfers); owner enters real transactions; verify v2 → v4 database upgrade

## M4.5 — Release

- [x] Cut `v0.4.0` (categories schema, Bank of Melbourne import); owner imports a real export; verify v4 → v6 upgrade

## M3 — Categories & rules

- [x] Category tree (groups + categories), defaults seeded by migration (core + commands)
- [x] Categories page: add, rename, move, archive groups and categories
- [x] Assign/bulk-assign categories in the transactions table
- [x] Auto-categorisation rules (payee/description contains, optional in/out → category; applied on import and on demand)

## M4 — Import (Bank of Melbourne first)

- [x] Get a Bank of Melbourne CSV header row + date/amount format from the owner (no real transactions) and write synthetic fixtures — see `src-tauri/core/tests/fixtures/bank_of_melbourne/README.md`
- [x] Bank of Melbourne CSV importer: parse + preview + commit (core + commands)
- [x] Import screen: choose file and account, preview with statuses, Import
- [x] Duplicate detection on import (import key = date + amount + description + occurrence; flags hand-entered look-alikes)
- [x] Generic CSV import with column mapping, saved as reusable presets
      (migration 0010 `import_presets`)
- [x] OFX/QFX and QIF import (OFX SGML + XML; QIF with an explicit date format)
- [ ] Optional: PDF statement import (for history beyond the online-banking export window)
- [ ] Open banking (CDR): revisit only if a way to receive data locally without a cloud intermediary becomes available — see DECISIONS

## M3.5 — Release

- [x] Cut `v0.5.0` (category assignment, Categories page, rules); verify v6 → v7 upgrade

## M5 — Budgets

- [x] Monthly budget per category: schema + commands
- [x] Budget page: budgeted vs spent, remaining, rollover option (per category, off by default)

## M5b — Valuations (super, shares, investments, property)

- [x] Valuation history per valued account (date + market value); balance = latest valuation
- [x] Quick "update value" action from the Accounts page

## M5b.5 — Release

- [ ] Cut `v0.10.0` (valuations); verify v9 → v11 upgrade

## M6 — Reports & dashboard

- [x] Dashboard: net worth, this month's spending, account balances
- [x] Spending by category (chart), income vs expense over time
- [x] Net worth over time

## M6.5 — Release

- [x] Cut `v0.6.0` (budgets, dashboard, reports); verify v7 → v9 upgrade

## M7 — Data safety

- [x] Cut `v0.7.0` (backup / restore); verify v9 → v9 upgrade (draft release
      built 2026-11-21; awaiting owner smoke test)
- [ ] Cut `v0.8.0` (CSV export); verify v9 → v9 upgrade

- [x] Backup / restore the database file from Settings (backups beside the
      database; restore keeps a pre-restore copy)
- [x] Export transactions to CSV (Transactions page, current filters)
- [ ] Auto-lock after inactivity (optional)

## M8 — v1.0

- [ ] App icon and branding
- [ ] Polish pass, keyboard shortcuts, empty states
- [ ] Cut `v1.0.0`
