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

**M0 — Cut v0.1.0** (first real installers):
1. Version is already `0.1.0` in `package.json`, `src-tauri/Cargo.toml`,
   `src-tauri/tauri.conf.json` — confirm, no bump needed for the first tag.
2. `git tag v0.1.0 && git push origin v0.1.0`; watch the **Release** workflow
   (`gh run list --workflow Release`). It has never run — expect to fix
   something (rpm tooling, bundle targets, NSIS/MSI on Windows).
3. Check the draft release has: `.exe` + `.msi` (Windows), `.AppImage`,
   `.deb`, `.rpm` (Linux).
4. Owner installs on Windows and Linux and runs the smoke test: first-run
   setup → lock → unlock (wrong then right) → change password → restart app →
   unlock with new password. This is the **first time the real desktop app
   runs** — everything so far was CI-compiled or mock-previewed.
5. Fix whatever the smoke test finds, then publish/keep the draft.

Then M1 — Accounts.

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
