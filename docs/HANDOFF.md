# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-20

## Current state

- Scaffold + CI green on Windows/Linux. Release workflow not yet exercised.
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
    `App.tsx` routes on vault status; unlocked view is still a placeholder
    with a Lock button.
  - Core tests pass locally; UI flow checked in a browser against a mocked
    backend (create → lock → wrong pw error → unlock). **Not yet run as a real
    desktop app** — needs the GTK/WebKit packages or the v0.1.0 installers.

## Next task

**M0 — UI shell:** sidebar navigation (Dashboard, Accounts, Transactions,
Budgets, Reports, Settings) with empty placeholder pages, shown only when
unlocked; Lock button in the sidebar footer. No router library needed yet —
a `page` state in `App.tsx` is enough; add `react-router` only if deep links
become necessary. Verify with the mocked-backend browser preview (see
desktop-app skill / AGENTS) before pushing.

After that: shared UI basics (incl. a TS `formatCents()` matching
`Money`'s Display: `$1,234.56`, `-$12.30`), change-password, v0.1.0.

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
