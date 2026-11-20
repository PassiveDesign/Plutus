# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-20

## Current state

- Tauri 2 + React/TS scaffold on `main`. Window shows "Plutus" + version from
  the Rust `app_info` command.
- CI and release workflows written. First CI run (after commit `6215b33`) was
  in progress at the end of this session — check its result first.
- No database, no features yet.

## Next task

1. Confirm CI green: `gh run list --workflow CI --limit 3`. If red, fix before
   anything else (likely suspects: clippy warnings, Windows-specific build issue).
2. Then: **M0 — SQLite layer** (see ROADMAP).

## Open questions for the owner

- Default currency (assumed AUD) and whether multi-currency is needed.
- Which banks/export formats to support first for import (CSV/OFX/QIF).
- Whether the database should be encrypted with a password (M7).

## Environment notes

- Dev host lacks GTK/WebKit `-dev` packages (owner will install later), so
  Rust compiles only in CI for now.
- `gh` lives at `~/.local/bin/gh`; Rust via rustup in `~/.cargo`.

## Half-done work

None.
