# CLAUDE.md — WebApps Project

## Project Overview

WebApps is a Tauri v2 desktop application that turns web apps into standalone desktop experiences. It uses Svelte+TypeScript for the shell UI and Rust for the backend.

## Tech Stack

- **Framework:** Tauri v2 (with `unstable` feature for multi-webview)
- **Frontend:** Svelte + TypeScript
- **Backend:** Rust
- **Config format:** TOML
- **Package manager:** npm (frontend), cargo (backend)

## Project Structure

```
webapps/
├── CLAUDE.md
├── docs/
│   ├── REQUIREMENTS.md
│   └── plans/
│       ├── 2026-02-18-webapps-design.md
│       └── 2026-02-18-phase1-implementation.md
├── src-tauri/                          # Rust backend
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── src/
│       ├── main.rs                     # Entry point (calls lib::run)
│       ├── lib.rs                      # Tauri builder setup, module declarations
│       ├── state.rs                    # AppState with Mutex-wrapped fields
│       ├── config/
│       │   ├── mod.rs
│       │   ├── models.rs              # GlobalConfig, SpaceConfig, AppConfig, IsolationMode
│       │   └── storage.rs             # TOML file I/O, config_dir(), webview_data_dir()
│       └── commands/
│           ├── mod.rs
│           ├── spaces.rs              # list, create, rename, delete, switch, set_isolation
│           ├── apps.rs                # add, remove, edit, reorder, get_apps_for_space
│           ├── webviews.rs            # open, switch_to, close, hide_all, get_active + badge parsing
│           └── favicon.rs             # fetch_site_info (title + favicon download)
├── src/                                # Svelte 5 + TypeScript frontend
│   ├── main.ts                        # Svelte mount point
│   ├── App.svelte                     # Root component, loads spaces, inits title listener
│   ├── vite-env.d.ts
│   └── lib/
│       ├── api.ts                     # Typed wrappers for all 19 Tauri IPC commands
│       ├── types/index.ts             # TS interfaces mirroring Rust models
│       ├── stores/
│       │   ├── spaces.ts             # spaces, activeSpaceId, activeSpace stores + actions
│       │   └── apps.ts               # activeAppId, notificationBadges stores + actions
│       └── components/
│           ├── Sidebar.svelte         # Main sidebar container with drag-and-drop
│           ├── SpaceSwitcher.svelte   # Space dropdown + create form
│           ├── AppItem.svelte         # App row with icon, badge, drag support
│           └── AddAppDialog.svelte    # URL input + fetch title dialog
├── index.html
├── package.json
├── vite.config.ts
├── svelte.config.js
├── tsconfig.json
└── tsconfig.node.json
```

## Key Architecture Decisions

1. **Single window, stacked webviews:** Each app is a separate Tauri webview created via `window.add_child()`. Only the active one is visible (others resized to 0x0). The sidebar is the main webview rendered by Svelte.
2. **Session isolation is configurable:** Shared per-space by default, with per-app override option.
3. **Bitwarden via CLI:** Uses `bw` subprocess for credential lookup. Session tokens stored in memory only.
4. **WebAuthn bridge:** JS polyfill intercepts `navigator.credentials`, forwards to Rust via IPC, which calls platform authenticator APIs.

## Development Commands

```bash
# Install dependencies
npm install

# Dev mode (hot reload)
npm run tauri dev

# Build for production (see "Local Installation & Releases" below)
npm run tauri -- build --no-bundle

# Run Rust tests
cd src-tauri && cargo test

# Run frontend linting
npm run lint
```

## Local Installation & Releases

The maintainer runs Arch Linux (CachyOS) and installs WebApps by copying the
binary — there is no package/installer step.

- **Installed binary:** `~/.local/bin/webapps`
- **Desktop entry:** `~/.local/share/applications/webapps.desktop`

### Quick install (recommended)

`scripts/install.sh` automates the whole flow below — build, verify the frontend
was embedded, install the binary, install icons, and write the desktop entry. It
detects a running instance and offers to kill it first.

```bash
./scripts/install.sh              # build + install (default)
./scripts/install.sh --skip-build # install an already-built binary
./scripts/install.sh --uninstall  # remove binary, icons, desktop entry
```

The manual steps below document what the script does, for reference.

### Building a production binary

You MUST build through the Tauri CLI. A bare `cargo build --release` does **not**
embed the frontend — the resulting binary keeps pointing at the dev server
(`http://localhost:1420`) and shows a blank screen once the dev server stops.
The CLI build is what flips Tauri into production asset-embedding mode.

```bash
# Produces src-tauri/target/release/webapps with the frontend embedded.
# --no-bundle skips deb/rpm/AppImage packaging (tauri.conf has "targets": "all");
# we only need the raw binary to copy.
npm run tauri -- build --no-bundle
```

Verify the frontend was embedded (should print an asset filename):

```bash
strings src-tauri/target/release/webapps | grep -o 'assets/index-[A-Za-z0-9_]*\.js'
```

### Updating the local copy (do this every time we merge to `main`)

After merging a change to `main`, rebuild and refresh the installed copy:

```bash
git checkout main
./scripts/install.sh   # or, manually:
# npm run tauri -- build --no-bundle
# cp -f src-tauri/target/release/webapps ~/.local/bin/webapps
```

**Close any running instance before launching the new one.** Two instances
sharing `~/.config/webapps/webview-data/` corrupt each other's WebKit state
(cookies/IndexedDB), which manifests as a garbled UI or broken sessions.

## Conventions

- **Rust:** Follow standard Rust conventions. Use `thiserror` for error types. Organize Tauri commands in `src-tauri/src/commands/` with one file per domain. All Tauri builder setup and module declarations go in `lib.rs` (NOT `main.rs`). `main.rs` just calls `webapps_lib::run()`.
- **Svelte:** Svelte 5 with TypeScript. Use `$state()`, `$derived()`, `$props()` runes in components. Use `svelte/store` (`writable`, `derived`) for cross-component shared state. Components in `src/lib/components/`.
- **HTTP client:** reqwest with `rustls-tls` (not native-tls, which has compatibility issues with recent Rust compilers).
- **Config:** All persistent config in TOML format under `~/.config/webapps/`.
- **Commits:** Conventional commits format (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`).
- **Error handling:** User-facing errors shown as toasts or inline messages. Backend errors logged and returned as `Result<T, String>` from Tauri commands.

## Phasing

- **Phase 1 (current):** Core app — Tauri scaffold, sidebar, Spaces, app management, webview lifecycle, config persistence
- **Phase 2:** Bitwarden CLI integration
- **Phase 3:** WebAuthn/Passkey bridge

## Important Notes

- Tauri multi-webview requires `unstable` feature flag in `Cargo.toml`
- WebKitGTK (Linux) does NOT support WebAuthn natively — hence the custom bridge in Phase 3
- Bitwarden CLI must be installed by the user; the app auto-detects its path
- Config directory: `~/.config/webapps/`
- System dependencies (Linux): `libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `libgtk-3-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
