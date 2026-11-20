# AGENTS.md — working on Plutus

Read this file first in every session, then `docs/HANDOFF.md`, then pick the
next unchecked task in `docs/ROADMAP.md`.

## What Plutus is

A local-first personal finance desktop app for Windows and Linux, for a single
user (the owner). Standalone installable program with its own native window —
never a browser-served UI. Private repo; updates installed manually from draft
GitHub Releases (no auto-updater).

## Stack

- Tauri 2 shell in `src-tauri/` (thin: commands + app state only).
- **`src-tauri/core/` = `plutus-core` crate: all storage and business logic.**
  No Tauri/GTK dependencies, so it builds and tests on any host:
  `cd src-tauri && cargo test -p plutus-core`. Put new logic here, not in the shell.
- React + TypeScript (Vite) UI in `src/`.
- Storage: SQLCipher-encrypted SQLite (`plutus.db`) in the OS app-data dir, unlocked by the user's password (`plutus_core::Database`).
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

## Shared checkout warning

More than one agent session may work on this repo at once, and they have
shared `~/projects/Plutus`. A session switching branches there carries other
sessions' uncommitted edits along (that is how transaction-command work ended
up inside the design PR). **Do your work in your own git worktree**
(`git worktree add -b <branch> ~/projects/Plutus-wt-<task> origin/main`),
never switch branches in the shared checkout, commit often, and remove the
worktree when done (`git worktree remove`).

## Conventions

- **Money is integer minor units** (`i64` cents) everywhere. Never floats. AUD only.
  Use `plutus_core::Money` (parse with `Money::parse`, display with `to_string()`
  → `$1,234.56`). It crosses to the UI as integer cents; keep values within
  `Money::MAX_CENTS` (JS safe-integer limit). Format only at the UI edge.
- **Dates:** store ISO-8601 (`YYYY-MM-DD` for transaction dates, UTC RFC 3339
  for timestamps).
- **DB schema changes only via numbered migrations** in `src-tauri/core/migrations/`, registered in
  `core/src/db/migrations.rs` (`MIGRATIONS`).
  Never edit a migration that has shipped in a release.
- **Rust owns the data.** The UI calls Tauri commands; it never touches SQLite
  directly. Data commands go through `with_db(&state.vault, |db| ...)` in
  `src-tauri/src/lib.rs` (blocking thread, returns `locked` if not unlocked).
  Errors reach the UI as `{ code, message, field? }`; use `splitError()` from
  `src/lib/api.ts` to put `field` errors on inputs and the rest in a toast.
- **Lists shared by Rust and TS** (account kinds, their order and
  classification) are pinned by a test on *both* sides. Change both together.
- **Tests:** business logic in Rust gets unit tests. Each roadmap task should
  leave `cargo test` and `npm run build` passing.
- **The look is Passive UI** (shared with PassiveSync; its own repo,
  `PassiveDesign/passive-ui`), vendored in `src/kit/` — never edit it there;
  see `src/kit/README.md` to update it. Style with its tokens in `App.css`
  (`var(--pane)`, `var(--muted)`, `var(--accent)`, `var(--bad)`, `var(--mono)`
  …), never raw colours, and follow its `GUIDE.md` (colour means attention;
  sans for words, mono for figures; hairlines, not cards). Timelines use
  `timeline` from `src/kit/passive-ui.mjs`.
- **UI building blocks live in `src/components/`** (`Button`, `TextField`,
  `Modal`, `DataTable` with `money` columns, `useToast`). Use them rather than
  raw elements so behaviour (a11y, focus, money formatting) stays consistent.
  Show money with `formatCents` or a `DataTable` money column, nowhere else.
- **Never commit real financial data**, sample exports, or `.env` files. Test
  fixtures must be synthetic.

## Commands

```sh
npm install
npm run tauri dev                 # run the app (needs Linux GTK/WebKit libs, see README)
npm run build                     # type-check + bundle UI
npm test                          # UI unit tests (Vitest)
cd src-tauri && cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cd src-tauri && cargo test -p plutus-core   # works on this host today (no GTK needed)
```

Note: the dev host (`~/projects/Plutus`) currently lacks the GTK/WebKit `-dev`
packages, so only `plutus-core` compiles locally; the Tauri shell compiles in
CI. Push and check `gh run watch` to verify shell changes until those are installed.
