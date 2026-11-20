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
- **2026-11-20 — Money as integer minor units (`i64`)**, never floating point.
- **2026-11-20 — Windows installers unsigned** for now (SmartScreen warning
  accepted; personal use).
- **2026-11-20 — Work is planned around usage limits:** small roadmap tasks,
  frequent pushes, HANDOFF.md updated at the end of every session. See AGENTS.md.
