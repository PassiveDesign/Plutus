# AGENTS.md — working on Plutus

Read this file first in every session, then `docs/HANDOFF.md`, then pick the
next unchecked task in `docs/ROADMAP.md`.

## What Plutus is

A local-first personal finance desktop app for Windows and Linux, for a single
user (the owner). Standalone installable program with its own native window —
never a browser-served UI. Private repo; updates installed manually from draft
GitHub Releases (no auto-updater).

## Stack

- Tauri 2 shell, Rust core in `src-tauri/`, React + TypeScript (Vite) UI in `src/`.
- Storage: SQLite in the OS app-data dir (never in the repo).
- CI: `.github/workflows/ci.yml` (Windows + Linux). Releases: push a `v*` tag.

## Session protocol (built for usage limits)

Work can be cut off at any moment. Leave the repo resumable at all times.

1. **Start:** read `AGENTS.md` → `docs/HANDOFF.md` → `docs/ROADMAP.md`.
   Check `git status` and `gh run list --limit 3` for leftover state.
2. **Pick one task** (a single `- [ ]` line in the roadmap). Tasks are sized to
   finish in one short session. If a task looks bigger, split it in the
   roadmap first and commit that.
3. **Work in small commits.** Commit and push after each meaningful step, not
   only at the end. Prefer Conventional Commits (`feat:`, `fix:`, `chore:`…).
4. **If interrupted mid-task:** push whatever exists to a `wip/<task>` branch
   and note it in HANDOFF. Never leave work only on the local disk.
5. **Finish:** tick the task in ROADMAP, rewrite HANDOFF (current state, next
   task, open questions, anything half-done), commit, push, confirm CI green.
6. **Decisions** that future sessions must respect go in `docs/DECISIONS.md`
   (one short entry each). Don't re-litigate decided items.

## Conventions

- **Money is integer minor units** (`i64` cents) everywhere. Never floats.
  Format only at the UI edge. Currency code stored alongside.
- **Dates:** store ISO-8601 (`YYYY-MM-DD` for transaction dates, UTC RFC 3339
  for timestamps).
- **DB schema changes only via numbered migrations** in `src-tauri/migrations/`.
  Never edit a migration that has shipped in a release.
- **Rust owns the data.** The UI calls Tauri commands; it never touches SQLite
  directly. Commands return `Result<T, String>`-style errors the UI can show.
- **Tests:** business logic in Rust gets unit tests. Each roadmap task should
  leave `cargo test` and `npm run build` passing.
- **Never commit real financial data**, sample exports, or `.env` files. Test
  fixtures must be synthetic.

## Commands

```sh
npm install
npm run tauri dev                 # run the app (needs Linux GTK/WebKit libs, see README)
npm run build                     # type-check + bundle UI
cd src-tauri && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Note: the dev host (`~/projects/Plutus`) currently lacks the GTK/WebKit `-dev`
packages, so Rust only compiles in CI. Push and check `gh run watch` to verify
Rust changes until those are installed.
