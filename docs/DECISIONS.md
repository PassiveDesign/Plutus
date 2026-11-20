# Decisions

Short, dated entries. Future sessions follow these without re-deciding.

- **2026-11-20 — Stack: Tauri 2 + Rust + React/TypeScript.** Standalone
  installable app with its own native window; no browser UI.
- **2026-11-20 — Platforms: Windows and Linux.** Installers: NSIS + MSI;
  AppImage, deb, rpm.
- **2026-11-20 — No auto-updater.** Repo stays private; new versions are
  installed manually from draft GitHub Releases.
- **2026-11-20 — Local-first storage in SQLite** in the OS app-data dir.
  No cloud sync.
- **2026-11-20 — Money as integer cents (`i64`)**, never floating point.
- **2026-11-20 — AUD only.** No multi-currency; no currency column needed
  beyond a fixed constant.
- **2026-11-20 — Password required at launch; whole database encrypted with
  SQLCipher.** The key is derived from the password. There is no recovery: a
  forgotten password means the data is unreadable (backups are encrypted too).
  Built into the DB layer from the start (M0), not bolted on later.
- **2026-11-20 — Import target: Bank of Melbourne** (Westpac group, same
  platform as St.George/BankSA). CSV export is the primary path. Personal
  online-banking exports are limited to a recent window (~3 months / 500
  transactions reported), so the owner should import regularly and duplicate
  detection must handle overlapping files.
- **2026-11-20 — Open banking (CDR) deferred.** CDR data can only flow to
  ACCC-accredited recipients (or their representatives); there is no
  machine-readable direct-to-consumer API. The only route is a paid cloud
  aggregator, which conflicts with local-first. Revisit if that changes.
- **2026-11-20 — Windows installers unsigned** for now (SmartScreen warning
  accepted; personal use).
- **2026-11-20 — Work is planned around usage limits:** small roadmap tasks,
  frequent pushes, HANDOFF.md updated at the end of every session. See AGENTS.md.
- **2026-11-20 — Logic lives in `plutus-core`** (`src-tauri/core`), a crate with
  no GUI dependencies; the Tauri crate is a thin shell. Keeps the core
  testable everywhere, including hosts without GTK/WebKit.
- **2026-11-20 — SQLCipher via `rusqlite` `bundled-sqlcipher-vendored-openssl`;
  key derivation is SQLCipher 4's default** (PBKDF2-HMAC-SHA512, 256k
  iterations, random per-file salt). Password passed as passphrase with
  `PRAGMA key`. Migrations are a small in-house runner on `PRAGMA user_version`
  (no extra crate).
