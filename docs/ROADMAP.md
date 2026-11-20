# Roadmap

Each `- [ ]` is one task, sized for a single short session. Work top to bottom
unless HANDOFF says otherwise. Tick tasks as they land on `main` with CI green.
Feature scope is a first draft — edit freely.

## M0 — Foundation

- [x] Repo, Tauri 2 + React/TS scaffold, CI + release workflows
- [x] Planning docs (AGENTS, ROADMAP, HANDOFF, DECISIONS)
- [ ] Confirm first CI run green on Windows + Linux
- [ ] SQLite layer: open DB in app-data dir, migration runner, first migration (`meta` table), unit tests on an in-memory DB
- [ ] Money type (`i64` minor units + currency) with parse/format helpers and tests
- [ ] UI shell: sidebar navigation (Dashboard, Accounts, Transactions, Budgets, Reports, Settings) with empty pages
- [ ] Shared UI basics: layout, theme (light/dark), form + table components, error toast
- [ ] Cut `v0.1.0` release; install on Windows and Linux to prove the pipeline

## M1 — Accounts

- [ ] Schema + Rust model: accounts (name, type, currency, opening balance, archived)
- [ ] Commands: create / list / update / archive account, with tests
- [ ] Accounts page: list with balances, add/edit form

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

## M4 — Import

- [ ] CSV import with column mapping + preview
- [ ] OFX/QFX import
- [ ] QIF import
- [ ] Duplicate detection on import

## M5 — Budgets

- [ ] Monthly budget per category: schema + commands
- [ ] Budget page: budgeted vs spent, remaining, rollover option

## M6 — Reports & dashboard

- [ ] Dashboard: net worth, this month's spending, account balances
- [ ] Spending by category (chart), income vs expense over time
- [ ] Net worth over time

## M7 — Data safety

- [ ] Backup / restore the database file from Settings
- [ ] Export transactions to CSV
- [ ] Optional database encryption (SQLCipher) with password unlock — decide first

## M8 — v1.0

- [ ] App icon and branding
- [ ] Polish pass, keyboard shortcuts, empty states
- [ ] Cut `v1.0.0`
