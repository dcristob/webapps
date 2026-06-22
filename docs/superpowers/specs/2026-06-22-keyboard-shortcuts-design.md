# Keyboard Shortcuts — Design

**Date:** 2026-06-22
**Status:** Approved (pre-implementation)
**Scope:** Phase 1 enhancement — keyboard navigation for apps, spaces, sidebar, and dialogs.

## Goal

Add keyboard shortcuts so users can drive WebApps without reaching for the mouse: cycle and jump to apps, toggle the sidebar, add an app, sleep the current app, and switch spaces via a searchable palette.

## Constraints & context

- **Multi-webview architecture.** The sidebar, the topbar, and each hosted app (Gmail, Slack, …) are separate webviews in one Tauri window. When a user is interacting with an app, that app's webview has focus — **not** the sidebar. A `keydown` listener in the sidebar alone catches almost nothing in practice.
- **WebApps-focused scope.** Shortcuts fire only when the WebApps window has keyboard focus. We do **not** register OS-level global hotkeys (no `tauri-plugin-global-shortcut`), so we never shadow bindings in other applications and add no new dependency.
- **The app already injects JS into every app webview** via `on_page_load` (`MEDIA_GUARD_JS`, link interceptor, `window.open` override). Shortcut capture reuses this mechanism.
- **Maintainer platform:** Arch Linux / WebKitGTK. Bindings are chosen to survive that environment (see below).

## Binding scheme

Plain `Ctrl`-based, matching Slack/Rambox/Franz desktop convention — the shell wins the key in the capture phase so hosted apps never see it. Two exceptions where plain Ctrl is technically broken on Linux:

| Shortcut | Action | Notes |
|---|---|---|
| `Ctrl+Tab` | cycle to next app (wrap) | |
| `Ctrl+Shift+Tab` | cycle to previous app (wrap) | |
| `Ctrl+1`–`Ctrl+9` | jump to app at 1-based sidebar position | no-op if out of range (positional jumps do not wrap) |
| `Ctrl+B` | toggle sidebar visibility | **not** `Ctrl+Space` — that is the ibus/fcitx IME-switch hotkey and is eaten before JS sees it. `Ctrl+B` reuses VS Code muscle memory. |
| `Ctrl+N` | open the Add App dialog | existing `show_dialog("add-app", active_space_id)` |
| `Ctrl+W` | **sleep** the current app | reversible webview destroy (frees memory, app stays in the space). Not the destructive "remove from space". |
| `Ctrl+Shift+S` | open space-switcher palette | **not** `Ctrl+Shift+Space` — same IME risk. Mnemonic: **S**witch space. |
| `Escape` | close dialog / palette | shell + dialog webviews only — see below. |

### Why two `Ctrl+Space` bindings were replaced

`Ctrl+Space` (and `Ctrl+Shift+Space`) are the default IME-switch hotkeys on most Linux multilingual setups (ibus, fcitx). The compositor/IME consumes them before the webview's `keydown` fires, so the shortcut would silently no-op for any IME user — including the maintainer's own Arch box. We avoid them rather than ship dead bindings.

### `Ctrl+W` semantics

Mapped to the **reversible** operation (`sleep_app_inner`), matching browser-Ctrl+W "close this tab" muscle memory. The destructive "remove app from space" remains a deliberate context-menu click — it should never be one keystroke away.

## Architecture (Approach A)

Two listener surfaces, one Rust dispatch command.

### 1. App-webview listener (injected)

A new `SHORTCUT_LISTENER_JS` is injected in `ensure_app_open`'s `on_page_load` (alongside `MEDIA_GUARD_JS`). It:

- binds `keydown` in the **capture phase**;
- matches the event against a small key→action table;
- on a hit, calls `e.preventDefault(); e.stopImmediatePropagation()` so the hosted app never reacts (the "shell wins" behavior users expect from app-launcher shells);
- then `invoke('handle_shortcut', { action })`.

**No input-field guard.** Every binding uses a modifier, so shortcuts fire correctly even while the user is typing in a text field — matching browser tab-switching expectations.

### 2. Shell-webview listener (Svelte)

A new `lib/shortcuts.ts` exports `installShellShortcuts()`, wired into `App.svelte`'s sidebar and topbar branches. It attaches a `window` `keydown` listener with the same table and calls the same `invoke('handle_shortcut')`. This covers the case where a shell webview has focus (e.g. immediately after clicking the "+" button).

### 3. Single dispatch point

`handle_shortcut(action: String)` in Rust resolves almost every action to an **existing** command. Only `toggle-sidebar` and the space-switcher trigger are new. The key→action table is duplicated in two places (the Rust-built injected JS for apps; a TS const in `lib/shortcuts.ts` for the shell), both mapping onto one closed set of action strings. Seven entries, rarely changed, kept in sync with a shared comment.

### Why not the rejected alternatives

- **B (inject apps only, emit Tauri event the sidebar acts on):** asymmetric — no shortcut fires while the sidebar itself has focus, so `Ctrl+Tab` right after clicking the sidebar silently fails. And more IPC hops than necessary.
- **C (GTK window-level `key-press-event` hook):** WebKitWebView consumes key events before the parent window sees them on Linux, so the window rarely gets them. Linux-only by nature, bypasses Tauri's abstraction. Fragile.

## Dispatch table

| Action | Backend behavior |
|---|---|
| `cycle-next` | next app in active space (wrap to first); slept apps wake via `open_app` |
| `cycle-prev` | previous app (wrap to last) |
| `jump-1`..`jump-9` | app at index N (1-based); **no-op if out of range** |
| `toggle-sidebar` | flip `sidebar_visible`, hide/show sidebar webview, reposition active app webview |
| `add-app` | `show_dialog("add-app", active_space_id)` |
| `sleep-app` | `sleep_app_inner` on active app, then switch to neighbor: the app immediately **after** it in list order; if the slept app was last, the one immediately **before**; if it was the only app, none. No wrap. |
| `space-switcher` | `open_space_switcher()` → new palette dialog |

### Escape — asymmetric by design

Escape is **only** handled by the shell/dialog Svelte listeners; it is absent from the injected app-webview table. A hosted app's own Escape (Gmail closing a draft, YouTube exiting fullscreen) must keep working. GTK context menus already dismiss on Escape natively, so they need no wiring. In shell/dialog contexts Escape calls `close_dialog()` if a dialog is open.

## Sidebar visibility (`Ctrl+B`)

- New persisted field `sidebar_visible: bool` on `GlobalConfig.general` (`#[serde(default)]` → `true`), mirrored as `sidebar_visible: Mutex<bool>` in `AppState`.
- `toggle_sidebar` flips the bool, hides/shows the sidebar webview, and resizes the active app webview to span full window width (hidden) or `width - sidebar_width` (shown).

### Layout helper

New `reposition_app_webviews(app_handle, state)` sets the **active** app webview's `x`/`width` from current `sidebar_visible` + `sidebar_width` + window logical size. It is called from three points so layout never desyncs:

1. `toggle_sidebar` (after flipping the bool),
2. `switch_to_app` (currently only `.show()`s without repositioning — would leave a gap when the sidebar is hidden),
3. `ensure_app_open` (initial `add_child` position uses current visibility).

On Linux the GTK hbox packing is the underlying reflow mechanism; explicit geometry is the cross-platform fallback. The helper is the single chokepoint either way.

## Space-switcher palette (`Ctrl+Shift+S`)

A new dialog type, **not** an in-shell overlay, so it works even when the sidebar is hidden and reuses the proven `show_dialog` path (centered, modal, transient, no decorations).

- **Sizing:** the palette is smaller than the 450×410 app/space dialogs. Rather than overload `show_dialog`'s hardcoded size, a dedicated `open_space_switcher()` command mirrors `show_dialog`'s centering/transient logic at ~420×360 and reuses the `"dialog"` label — so it still honors the one-dialog rule (if a dialog is already open, it focuses that dialog rather than clobbering it).
- **Component:** new `SpaceSwitcherPalette.svelte`, mounted from `App.svelte` when `dialogMode === "space-switcher"`. Uses the existing `spaces` / `activeSpaceId` stores — no new data plumbing.
  - **Search input** (autofocus), case-insensitive substring filter on space name; empty query lists all spaces.
  - **Row:** color dot (`space.space.color`) + name + app count (e.g. `6 apps`); the active space is visually marked.
  - **Keys:** `ArrowDown`/`ArrowUp` move selection (wraps); `Enter` switches; `Escape` closes; clicking a row equals `Enter` on it.
  - **On switch:** `switchToSpace(id)` (existing store action → `switch_space` + `hideAllAppWebviews`) → `emit("space-switched")` (existing reload trigger) → `focus_active_app()` (new: `set_focus()` on the active app webview so typing resumes in the app, not the now-closed palette) → `closeDialog()`.
- **Scope:** spaces only for v1. A unified space+app palette is a natural follow-up but is out of scope here.

## Edge cases

- Cycle / jump / sleep with no apps or no active app → no-op.
- Any shortcut while a dialog is already open → the one-dialog guard focuses the open dialog (consistent with today's `show_dialog`); the palette will not clobber an open Add-App dialog.
- Toggle sidebar with no app open → just hides/shows the sidebar; nothing to reposition.
- Sidebar visibility persists across restarts (stored in config).
- Rapid `Ctrl+Tab` → ordered processing in Rust; no race.

## Testing

Extract the pure logic as free functions in `commands/shortcuts.rs` so it is unit-testable with `cargo test`:

- `cycle_index(current, len, dir)` — wrap math.
- `neighbor_after_sleep(apps, removed_id)` — app after the removed one (prev if last, none if only). No wrap.
- `jump_index(n, len) -> Option<usize>` — no-wrap.

`handle_shortcut` composes these with the `AppHandle`. The injected-JS matching, dialog flow, and sidebar toggle have no headless webview harness, so they are verified manually (plus `npm run lint` / typecheck on the frontend).

## File-level changes

### Backend (Rust)

- `commands/shortcuts.rs` (**new**) — `handle_shortcut` command; pure helpers (`cycle_index`, `neighbor_after_sleep`, `jump_index`); `SHORTCUT_LISTENER_JS` builder.
- `commands/webviews.rs` — `reposition_app_webviews`; wire it into `switch_to_app` and `ensure_app_open`; inject `SHORTCUT_LISTENER_JS` in `on_page_load`.
- `commands/dialog.rs` — `open_space_switcher`, `focus_active_app`.
- `state.rs` — `sidebar_visible: Mutex<bool>`.
- `config/models.rs` — `sidebar_visible` on `GlobalConfig` (`#[serde(default)]` → true).
- `commands/mod.rs` — declare `shortcuts`.
- `lib.rs` — init `sidebar_visible` from config; register `handle_shortcut`, `toggle_sidebar`, `open_space_switcher`, `focus_active_app` in `invoke_handler!`.

### Frontend (TS/Svelte)

- `lib/shortcuts.ts` (**new**) — `installShellShortcuts()` + binding table → `invoke('handle_shortcut')`.
- `lib/components/SpaceSwitcherPalette.svelte` (**new**) — the palette.
- `lib/api.ts` — `handleShortcut`, `openSpaceSwitcher`, `focusActiveApp` wrappers.
- `App.svelte` — call `installShellShortcuts()` in sidebar/topbar branches; add `dialogMode === "space-switcher"` branch rendering `<SpaceSwitcherPalette />`.

### Capabilities

Register the new commands in `invoke_handler!` and confirm `src-tauri/capabilities/default.json` permits them.

## Out of scope

- System-wide global shortcuts (`tauri-plugin-global-shortcut`).
- User-configurable / remappable bindings (the table is closed for v1).
- A unified space+app command palette.
- Reposition-on-window-resize for app webviews (a pre-existing gap; the helper makes it cheap to add later but it is not part of this work).
- Shortcuts that fire while a hosted app's webview does **not** pass them through at the engine level (e.g. any remaining WebKitGTK/compositor intercepts beyond the IME cases already addressed).
