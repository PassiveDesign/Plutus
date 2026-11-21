# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-21

## Current state

### Newest first

- **v0.7.0 built as a draft release** (run 44894810100, both platforms green):
  `Plutus_0.7.0_x64-setup.exe` 3.5 MB, `.msi` 4.7 MB, `.AppImage` 79.8 MB,
  `.deb` 5.2 MB, `.rpm` 5.2 MB. CI 44894354054: 125 core, 17 shell, 153 UI
  tests. Awaiting the owner's smoke test; publish with
  `gh release edit v0.7.0 --draft=false --latest` once it passes.
- **M7 backup/restore done** (commit `1f23523` and earlier): copies are byte
  copies of the encrypted database kept beside it (newest 10); Settings has
  Back up now / Restore with a confirmation; restore leaves
  `plutus.db.before-restore` behind. Core `backup` module + 4 commands.
  The shell crate has no `chrono`; core supplies the timestamp
  (`backup_now()`; `backup_at()` for tests). Core compiles here; the shell
  only in CI, so shell-only mistakes surface in CI.
- **v0.6.0 draft** still exists and is superseded by v0.7.0; delete it when
  convenient.

## Next task

1. Owner smoke test of v0.7.0: install over v0.5.0, unlock (v7 → v9), check
   budgets, dashboard, reports, then Settings → Backup → Back up now and
   Restore. If good, publish the release
   (`gh release edit v0.7.0 --draft=false --latest`) and tick the ROADMAP item.
2. Then the rest of M7: CSV export of transactions, optional auto-lock after
   inactivity. Ask the owner which first.

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
