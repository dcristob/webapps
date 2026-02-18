# WebApps — Requirements Document

## Project Summary

A Rust-based desktop application that converts web applications into standalone desktop experiences. Built with Tauri v2 and Svelte+TypeScript. Inspired by [WebCatalog](https://webcatalog.io/en/desktop).

## Functional Requirements

### FR-1: App Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.1 | Users can add a web app by entering a URL | Must |
| FR-1.2 | App auto-fetches favicon and page title on add | Must |
| FR-1.3 | Users can customize app name and icon | Must |
| FR-1.4 | Users can remove an app (with optional data deletion) | Must |
| FR-1.5 | Users can edit app properties (name, URL, icon, isolation) | Must |
| FR-1.6 | Users can reorder apps via drag-and-drop | Should |

### FR-2: Spaces (Workspaces)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.1 | Users can create named Spaces | Must |
| FR-2.2 | Users can rename and delete Spaces | Must |
| FR-2.3 | Users can switch between Spaces in the sidebar | Must |
| FR-2.4 | A default "General" Space exists on first launch | Must |
| FR-2.5 | Only one Space is visible at a time | Must |
| FR-2.6 | Each Space maintains its own ordered list of apps | Must |

### FR-3: Session Isolation

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.1 | Apps in a Space share cookies/storage by default | Must |
| FR-3.2 | Users can set a Space to per-app isolation mode | Must |
| FR-3.3 | Individual apps can override Space isolation setting | Must |
| FR-3.4 | Session data stored in `~/.config/webapps/webview-data/` | Must |

### FR-4: Sidebar & Navigation

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.1 | Left sidebar displays current Space's apps with icons | Must |
| FR-4.2 | Space switcher at top of sidebar | Must |
| FR-4.3 | "Add App" button in sidebar | Must |
| FR-4.4 | Settings access from sidebar | Must |
| FR-4.5 | Active app visually highlighted | Must |
| FR-4.6 | Notification badges from webview title changes | Should |

### FR-5: Bitwarden Integration (Phase 2)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.1 | Auto-detect Bitwarden CLI (`bw`) path | Must |
| FR-5.2 | Users can manually set CLI path in settings | Must |
| FR-5.3 | Vault unlock via master password prompt | Must |
| FR-5.4 | Session token stored in memory only | Must |
| FR-5.5 | Autofill via `Ctrl+Shift+L` keyboard shortcut | Must |
| FR-5.6 | Credential lookup by current app URL | Must |
| FR-5.7 | Multi-match picker when multiple credentials found | Must |
| FR-5.8 | Configurable session timeout | Should |

### FR-6: WebAuthn/Passkey Bridge (Phase 3)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-6.1 | Intercept `navigator.credentials` API calls in webviews | Must |
| FR-6.2 | Forward WebAuthn requests to Rust backend via IPC | Must |
| FR-6.3 | Support platform authenticator APIs (Linux D-Bus, macOS, Windows) | Must |
| FR-6.4 | Support USB security keys via libfido2 fallback | Should |
| FR-6.5 | Support mobile passkeys via platform hybrid transport | Should |
| FR-6.6 | Handle both attestation and assertion flows | Must |

## Non-Functional Requirements

### NFR-1: Performance

| ID | Requirement |
|----|-------------|
| NFR-1.1 | App startup in under 3 seconds |
| NFR-1.2 | App switching feels instant (webviews stay loaded) |
| NFR-1.3 | Memory usage proportional to number of loaded webviews |

### NFR-2: Security

| ID | Requirement |
|----|-------------|
| NFR-2.1 | Bitwarden session tokens never written to disk |
| NFR-2.2 | WebAuthn bridge must correctly implement FIDO2 flows |
| NFR-2.3 | Webview data directories properly isolated per configuration |

### NFR-3: Usability

| ID | Requirement |
|----|-------------|
| NFR-3.1 | Single-window experience (no popup windows for app switching) |
| NFR-3.2 | Dark theme support |
| NFR-3.3 | Keyboard shortcuts for common actions |

### NFR-4: Platform

| ID | Requirement |
|----|-------------|
| NFR-4.1 | Primary target: Linux (x86_64) |
| NFR-4.2 | Secondary targets: macOS, Windows |
| NFR-4.3 | WebKitGTK as webview engine on Linux |

## Technical Constraints

- Tauri v2 multi-webview requires the `unstable` Cargo feature flag
- WebKitGTK does not natively support WebAuthn (hence custom bridge)
- Bitwarden CLI must be installed separately by the user
- Platform authenticator API support varies by OS and version

## Phasing

| Phase | Scope | Dependencies |
|-------|-------|-------------|
| Phase 1 | Core app: Tauri scaffold, sidebar, Spaces, app management, webview lifecycle, config persistence | None |
| Phase 2 | Bitwarden CLI integration: detection, unlock, autofill, picker | Phase 1 |
| Phase 3 | WebAuthn bridge: JS polyfill, platform APIs, libfido2, mobile passkeys | Phase 1 |
