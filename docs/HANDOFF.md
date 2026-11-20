# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-20

## Current state

- Scaffold + CI green on Windows/Linux. Release workflow not yet exercised.
- **UI shell done:** `src/shell/` — `pages.ts` (page registry: id, label,
  icon, summary, milestone), `Sidebar.tsx` (nav + Settings/Lock footer +
  version), `AppShell.tsx` (page state, Ctrl+L locks). Every page is
  `src/pages/PlaceholderPage.tsx` for now. To build a real page: create
  `src/pages/<Name>Page.tsx` and switch on `current` in `AppShell`.
  Light + dark themes via CSS variables in `App.css`. Checked in the mocked
  browser preview (nav, active state, dark mode, Ctrl+L, Lock button).
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

**M0 — Change password** (Settings page):
- Core: `Vault::change_password(current, new)` — require unlocked, verify
  `current` (open a second connection with it, or compare via a fresh
  `Database::open`), `check_new_password(new)`, then `Database::change_password`
  (rekey). Tests: wrong current rejected, short new rejected, old pw stops
  working, new works after lock/unlock.
- Shell: `change_password(current, new)` command via `spawn_blocking`.
- UI: `src/pages/SettingsPage.tsx` replacing the placeholder for `settings`
  (switch in `AppShell`): form with current / new / confirm (TextField),
  no-recovery reminder, success toast. Screen tests with mocked API.

Then: cut **v0.1.0** (bump versions in 3 files, tag, watch release workflow,
download installers and test on Windows + Linux).

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
