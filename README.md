# Plutus

A local-first personal finance application for Windows and Linux.

Plutus is a standalone desktop program: it installs like any other app, runs in
its own native window, and keeps all financial data on your machine.

## Goals

- Track accounts, transactions, and balances
- Budgeting and categorisation
- Reports and insights on spending and saving
- Local-first: your financial data never leaves your machine
- Native installers for Windows and Linux

## Tech stack

- **[Tauri 2](https://tauri.app)** — native app shell and installers
- **Rust** — application core, data storage, business logic (`src-tauri/`)
- **React + TypeScript (Vite)** — user interface, rendered in the app's own
  window (`src/`)

## Development

### Prerequisites

- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) LTS + npm
- Platform libraries:
  - **Linux (Debian/Ubuntu):**
    ```sh
    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
      libayatana-appindicator3-dev librsvg2-dev libssl-dev pkg-config build-essential
    ```
  - **Windows:** Microsoft C++ Build Tools and WebView2 (preinstalled on Windows 10/11)

### Run

```sh
npm install
npm run tauri dev      # run the app with hot reload
```

### Test

```sh
npm run build                       # type-check + bundle the UI
cd src-tauri && cargo test          # Rust tests
```

### Build installers locally

```sh
npm run tauri build
```

Output goes to `src-tauri/target/release/bundle/`.

## Releases

Releases are built by GitHub Actions. To cut one:

1. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
   `package.json`.
2. Commit, then tag and push:
   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The **Release** workflow builds the installers and attaches them to a draft
   release:
   - **Windows:** `.exe` (NSIS) and `.msi`
   - **Linux:** `.AppImage`, `.deb`, `.rpm`
4. Download the installer for your platform from the release page and install
   it over the previous version.

The repository is private, so updates are installed manually. There is no
auto-updater.
