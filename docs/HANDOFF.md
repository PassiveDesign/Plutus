# Handoff

Rewritten at the end of every session. Read after AGENTS.md.

**Last updated:** 2026-11-21

## Current state

### Newest first

- **Generic CSV import + presets done.** `plutus-core::import::generic`
  (`headers`, `shape`, `guess_mapping`, `validate`, `parse`) and
  `import::presets` (list/save/delete, migration 0010); commands `import_shape`,
  `preview_generic_import`, `list_import_presets`, `save_import_preset`,
  `delete_import_preset`. `ImportPage` picks the importer from the header row:
  Bank of Melbourne's exact columns use the existing parser, anything else
  opens a "Map the columns" form (with date format, amount vs debit/credit,
  optional payee/balance) that can be saved as a named preset. 143 core,
  20 shell, 161 UI tests.

- **M7 CSV export done.** `plutus-core::export` writes the transactions matching the filters to `exports/transactions-<stamp>.csv` (plain text, signed amounts, oldest first). Command `export_transactions`; "Export CSV" button on the Transactions page. 130 core tests (5 new), 157 UI tests (4 new); checked in the browser preview. `errorMessage` now returns the message for plain `Error` objects instead of "Error: …".
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

1. **v0.9.0 (generic CSV import)** is a draft release: `AppImage` 108.4 MB and
   `.deb` 5.3 MB, built on the self-hosted runner (run 44905996011). Owner
   smoke test, then publish with `gh release edit v0.9.0 --draft=false --latest`.
   It supersedes v0.8.0; delete the v0.8.0 draft afterwards.
2. **Windows installers are not being built.** A Tauri Windows build cannot be
   cross-compiled from Linux, so either restore GitHub billing and set the repo
   variable `WINDOWS_CI=true`, or register a second runner on a Windows machine.
3. Remaining M7: auto-lock after idle (optional; the owner has not asked).
4. Remaining M4: OFX/QFX and QIF import, then optional PDF statements.

## Open questions for the owner

- Bank of Melbourne CSV format: need the header row and one example of the
  date/amount format (made-up values are fine) before the M4 importer.
- Does the export menu also offer OFX/QFX/QIF?

## Environment notes

- The dev host now HAS the GTK/WebKit dev packages (webkit2gtk-4.1, gtk+-3.0
  and libsoup-3.0 all resolve in pkg-config), so the Tauri shell crate can be
  checked locally: `cd src-tauri && cargo clippy -p plutus --all-targets -- -D
  warnings`. The first run builds Tauri and is slow, but it catches
  shell-only mistakes before they cost a CI round trip. `cargo test -p
  plutus-core` stays the fast inner loop.
- Cold `plutus-core` build takes ~80s (SQLCipher + vendored OpenSSL).
- `gh` lives at `~/.local/bin/gh`; Rust via rustup in `~/.cargo`.

## CI setup (self-hosted runner)

- GitHub-hosted Actions minutes are blocked on this account ("recent account
  payments have failed or your spending limit needs to be increased"), so Linux
  CI and Linux release builds run on a **self-hosted runner** instead:
  - Directory `~/actions-runner-plutus`, runner name `plutus-local`, labels
    `self-hosted, linux, x64, plutus`, version 2.338.2.
  - Start it with `cd ~/actions-runner-plutus && ./run.sh` (it is not a systemd
    service, so it does not survive a reboot; restart it after booting).
  - It runs as the owner's user and executes repo code — fine for this private
    single-author repo, not for untrusted forks.
  - Check its state with `gh api repos/PassiveDesign/Plutus/actions/runners`.
- Windows jobs are gated on the repo variable `WINDOWS_CI == 'true'` and are
  skipped while it is unset. Set it once billing works:
  `gh variable set WINDOWS_CI --body true`.
- The self-hosted runner has no `rpm` toolchain (installing it needs sudo), so
  the Linux release job passes `--bundles deb,appimage`; `rpm` stays in
  `tauri.conf.json` for hosted builds.
- An empty commit `8cd8f53` sits on `main` from the billing diagnosis.

## Half-done work

None.
