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
- [ ] Offset linking in the UI: pick a loan for an offset account; loan row shows its offsets and the net (loan + offsets) owed

## M2 — Transactions

- [ ] Schema + model: transactions (account, date, payee, amount, memo, category, cleared)
- [ ] Commands: add / edit / delete / list (paged, by account, by date range), with tests
- [ ] Running balance computation, with tests
- [ ] Transactions page: table, filters, add/edit dialog
- [ ] Transfers between accounts (paired transactions)

## M3 — Categories & rules

- [ ] Category tree (groups + categories), defaults seeded by migration
- [ ] Assign/bulk-assign categories in the transactions table
- [ ] Auto-categorisation rules (payee contains / amount matches → category)

## M4 — Import (Bank of Melbourne first)

- [ ] Get a Bank of Melbourne CSV header row + date/amount format from the owner (no real transactions) and write synthetic fixtures
- [ ] Bank of Melbourne CSV importer (built-in preset) with preview before commit
- [ ] Duplicate detection on import (date + amount + description hash; overlapping exports are expected)
- [ ] Generic CSV import with column mapping, saved as reusable presets
- [ ] OFX/QFX and QIF import (only if the bank's export menu offers them)
- [ ] Optional: PDF statement import (for history beyond the online-banking export window)
- [ ] Open banking (CDR): revisit only if a way to receive data locally without a cloud intermediary becomes available — see DECISIONS

## M5 — Budgets

- [ ] Monthly budget per category: schema + commands
- [ ] Budget page: budgeted vs spent, remaining, rollover option

## M5b — Valuations (super, shares, investments, property)

- [ ] Valuation history per valued account (date + market value); balance = latest valuation
- [ ] Quick "update value" action from the Accounts page

## M6 — Reports & dashboard

- [ ] Dashboard: net worth, this month's spending, account balances
- [ ] Spending by category (chart), income vs expense over time
- [ ] Net worth over time

## M7 — Data safety

- [ ] Backup / restore the database file from Settings
- [ ] Export transactions to CSV
- [ ] Auto-lock after inactivity (optional)

## M8 — v1.0

- [ ] App icon and branding
- [ ] Polish pass, keyboard shortcuts, empty states
- [ ] Cut `v1.0.0`
