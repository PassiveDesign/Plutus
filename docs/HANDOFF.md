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
- No database, no features yet.

## Next task

**M0 — Encrypted SQLite layer** (see ROADMAP).

## Open questions for the owner

- Bank of Melbourne CSV format: need the header row and one example of the
  date/amount format (made-up values are fine) before the M4 importer.
- Does the export menu also offer OFX/QFX/QIF?

## Environment notes

- Dev host lacks GTK/WebKit `-dev` packages (owner will install later), so
  Rust compiles only in CI for now.
- `gh` lives at `~/.local/bin/gh`; Rust via rustup in `~/.cargo`.

## Half-done work

None.
