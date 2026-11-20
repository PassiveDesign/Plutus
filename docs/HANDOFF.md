# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-20

## Current state

- Tauri 2 + React/TS scaffold on `main`. Window shows "Plutus" + version from
  the Rust `app_info` command.
- CI green on Windows + Linux (run 44684350305: fmt, clippy, tests, UI build).
- Release workflow written but not yet exercised (first tag is `v0.1.0`, end of M0).
- Planning docs and owner decisions recorded (AUD only, SQLCipher password
  lock, Bank of Melbourne CSV import, open banking deferred).
- **Encrypted DB layer done** in `plutus-core` (`src-tauri/core`):
  `Database::create/open/change_password`, migration runner, migration 0001
  (`meta` table). 12 unit tests pass locally (wrong password, file really
  encrypted, quotes/unicode passwords, rekey, no-overwrite, idempotent migrations).
- Not yet wired into the Tauri app — that's the next task.

## Next task

**M0 — Password flow** (see ROADMAP): Tauri app state holding
`Mutex<Option<Database>>`, commands `vault_status` (new/locked/unlocked),
`create_vault(password)`, `unlock(password)`, `lock()`; DB path from
`app.path().app_data_dir()` + `Database::path_in`. UI: create-password screen
(confirm field + no-recovery warning) and unlock screen; app content only
renders when unlocked. Run blocking DB open off the main thread
(`tauri::async_runtime::spawn_blocking`) — PBKDF2 takes a moment.

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
