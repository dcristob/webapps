# WebApps — Design Document

**Date:** 2026-02-18
**Status:** Approved

## Overview

WebApps is a Rust-based desktop application that turns web applications into standalone desktop experiences, similar to [WebCatalog](https://webcatalog.io/en/desktop). Users add web apps by URL, organize them into Spaces, and interact with them through a unified single-window interface with a sidebar. The app includes Bitwarden CLI integration for password autofill and a custom WebAuthn bridge for passkey support.

## Architecture

### Stack

- **Runtime:** Tauri v2 (with `unstable` feature for multi-webview support)
- **Shell UI:** Svelte + TypeScript
- **Backend:** Rust
- **Storage:** TOML config files
- **Password manager:** Bitwarden CLI (`bw`) subprocess
- **Passkeys:** Platform authenticator APIs + libfido2 fallback

### Window Layout

Single OS window containing:

```
┌──────────┬─────────────────────────────────────┐
│ Sidebar  │         App Content Area             │
│ Webview  │    (Stacked webviews, one visible)   │
│          │                                      │
│ Spaces:  │    ┌─────────────────────────────┐   │
│ ▼ Work   │    │     Active App Webview      │   │
│   Gmail  │    │     (e.g., Gmail)           │   │
│   Slack  │    │                             │   │
│ ▼ Personal│   │     Hidden: Slack, Discord  │   │
│   YouTube│    │     (loaded but invisible)   │   │
│          │    └─────────────────────────────┘   │
│ [+] Add  │                                      │
│ ⚙ Settings│                                     │
└──────────┴─────────────────────────────────────┘
```

- **Sidebar:** Svelte+TS webview pinned to the left, renders Space/app navigation.
- **App content:** Each added app is a separate Tauri webview, created via `WebviewBuilder`. Only the active app's webview is visible; others are hidden but remain loaded.
- **Communication:** Sidebar communicates with the Rust backend via Tauri IPC commands. Backend manages webview lifecycle (create, show, hide, destroy).

### Data Flow

1. **User adds app** → Sidebar sends IPC command → Rust backend creates webview with the URL → Updates TOML config → Sidebar refreshes
2. **User switches app** → Sidebar sends IPC → Backend hides current webview, shows target webview
3. **User triggers autofill** → Keyboard shortcut intercepted → Backend calls `bw` CLI → Credentials returned → Injected into active webview via JS
4. **Passkey request** → Injected JS polyfill intercepts `navigator.credentials` → IPC to backend → Platform authenticator API called → Result returned to JS

## Data Model & Storage

### Directory Structure

```
~/.config/webapps/
├── config.toml              # Global settings
├── spaces/
│   ├── work.toml            # Space definition + app list
│   └── personal.toml
└── webview-data/
    ├── space-work/          # Shared cookies/storage per space
    │   ├── gmail/           # Per-app data (if isolation override)
    │   └── slack/
    └── space-personal/
        └── youtube/
```

### config.toml

```toml
[general]
sidebar_width = 250
theme = "dark"

[bitwarden]
cli_path = "/usr/bin/bw"    # Auto-detected or manual
session_timeout_minutes = 30

[webauthn]
enabled = true
```

### spaces/<name>.toml

```toml
[space]
name = "Work"
icon = "briefcase"
isolation = "shared"  # "shared" (default) or "per-app"

[[apps]]
id = "gmail-work"
name = "Gmail"
url = "https://mail.google.com"
icon = "auto"  # auto-fetched favicon, or custom icon path
isolation_override = false

[[apps]]
id = "slack"
name = "Slack"
url = "https://app.slack.com"
icon = "auto"
```

### Session Isolation

- **Shared (default):** All apps in a Space share one `webview-data/space-<name>/` directory for cookies, localStorage, etc. Passed to `WebviewBuilder::data_directory()`.
- **Per-app isolation:** Each app gets its own subdirectory under the space directory.
- **Override:** Individual apps can opt into per-app isolation even when the Space uses shared mode, via `isolation_override = true`.

## Core Features

### App Management

- **Add:** User enters URL. App auto-fetches favicon and page title via HTTP request. User can customize name and icon. App added to current Space.
- **Remove:** Right-click context menu or settings panel. Optionally deletes associated webview data.
- **Reorder:** Drag-and-drop in sidebar.
- **Edit:** Change name, URL, icon, isolation setting via context menu or settings.

### Spaces

- Create, rename, and delete Spaces from the sidebar.
- Switch Spaces via sidebar. Switching shows that Space's apps and hides others.
- A "General" Space exists by default.
- Only one Space is visible at a time.

### Sidebar

- Left sidebar showing the current Space's apps with icons.
- Space switcher at the top (dropdown or tabs).
- "Add App" button at the bottom.
- Settings gear icon.
- Active app highlighted.
- **Notification badges:** Parsed from webview title changes (e.g., "(3) Gmail" → badge shows 3).

### Bitwarden Integration

- **Setup:** Auto-detect `bw` CLI path; user can override in settings.
- **Unlock:** When autofill is triggered and vault is locked, prompt for master password via modal dialog. Session token stored in memory only (never persisted to disk).
- **Autofill flow:**
  1. User presses `Ctrl+Shift+L`
  2. Rust backend calls `bw list items --url <current_url> --session <token>`
  3. If multiple matches, show picker UI
  4. Inject credentials into focused form fields via JavaScript
- **Session timeout:** Configurable; token cleared after inactivity period.

### WebAuthn/Passkey Bridge

- **JavaScript polyfill** injected into every app webview that intercepts `navigator.credentials.create()` and `navigator.credentials.get()`.
- Intercepted calls forwarded to Rust backend via Tauri IPC.
- **Primary:** Platform authenticator API per OS:
  - Linux: XDG Desktop Portal / D-Bus
  - macOS: AuthenticationServices framework
  - Windows: Windows Hello / WebAuthn API
- **Fallback:** `libfido2` for USB security keys.
- Platform APIs handle the hybrid/BLE/QR flow for mobile passkeys natively.
- **This is a Phase 3 feature** due to complexity and security criticality.

## Error Handling

- **App fails to load:** Error page in webview with "Reload" and "Edit URL" buttons.
- **Bitwarden CLI not found:** Warning in settings with install instructions. Autofill shortcut shows toast notification.
- **Vault locked:** Inline modal for master password.
- **No authenticator:** User-friendly error message suggesting connecting a key or using phone.
- **WebView crash:** Log error, offer reload.
- **Config corruption:** Keep backup of last valid config; restore on parse failure.

## Phasing

### Phase 1 — Core App (MVP)

- Tauri v2 project scaffolding
- Single-window with sidebar (Svelte+TS)
- Add/remove/edit apps (manual URL entry)
- Spaces (create, switch, manage)
- Webview management (create, show/hide, data isolation)
- Notification badges from title changes
- App config persistence (TOML files)
- Drag-and-drop reorder

### Phase 2 — Bitwarden Integration

- Bitwarden CLI detection and configuration
- Vault unlock/session management
- Autofill keyboard shortcut and credential injection
- Multi-match picker UI
- Session timeout

### Phase 3 — WebAuthn/Passkey Bridge

- JavaScript polyfill injection for `navigator.credentials`
- Platform authenticator API integration (Linux first)
- libfido2 fallback for USB keys
- Mobile passkey support via platform hybrid transport
- Attestation and assertion flow handling

## Non-Goals (Explicitly Out of Scope)

- App catalog / discovery (users add by URL only)
- Cloud sync
- Ad/tracker blocking (can be added later)
- Mobile (iOS/Android) support
- Built-in browser dev tools
- Multi-account management beyond what Spaces + isolation provides
