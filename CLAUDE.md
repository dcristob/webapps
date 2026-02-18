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
│       └── 2026-02-18-webapps-design.md
├── src-tauri/           # Rust backend
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/    # Tauri IPC commands
│   │   ├── config/      # TOML config management
│   │   ├── webview/     # Webview lifecycle management
│   │   ├── bitwarden/   # Bitwarden CLI integration
│   │   └── webauthn/    # WebAuthn bridge
│   └── tauri.conf.json
├── src/                 # Svelte frontend (shell UI)
│   ├── App.svelte
│   ├── lib/
│   │   ├── components/  # Sidebar, SpaceSwitcher, AppIcon, etc.
│   │   ├── stores/      # Svelte stores for app/space state
│   │   └── types/       # TypeScript type definitions
│   └── main.ts
├── package.json
└── vite.config.ts
```

## Key Architecture Decisions

1. **Single window, stacked webviews:** Each app is a separate Tauri webview. Only the active one is visible. Sidebar is its own webview.
2. **Session isolation is configurable:** Shared per-space by default, with per-app override option.
3. **Bitwarden via CLI:** Uses `bw` subprocess for credential lookup. Session tokens stored in memory only.
4. **WebAuthn bridge:** JS polyfill intercepts `navigator.credentials`, forwards to Rust via IPC, which calls platform authenticator APIs.

## Development Commands

```bash
# Install dependencies
npm install

# Dev mode (hot reload)
npm run tauri dev

# Build for production
npm run tauri build

# Run Rust tests
cd src-tauri && cargo test

# Run frontend linting
npm run lint
```

## Conventions

- **Rust:** Follow standard Rust conventions. Use `thiserror` for error types. Organize Tauri commands in `src-tauri/src/commands/` with one file per domain (apps, spaces, bitwarden, webauthn).
- **Svelte:** Use TypeScript for all `.svelte` and `.ts` files. Use Svelte stores for state management. Components in `src/lib/components/`.
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
