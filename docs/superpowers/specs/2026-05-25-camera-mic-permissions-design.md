# Camera & Microphone Permissions — Design

**Date:** 2026-05-25
**Status:** Draft
**Scope:** Linux (WebKitGTK). Other platforms out of scope until later phases.

## Goal

Allow apps to use the camera and microphone, but only with the user's explicit consent. Surface clear, always-visible UI showing the permission state and current capture activity, and let the user revoke or re-grant the permission at any time.

## User-facing behavior

1. When an app calls `getUserMedia` for the first time, an **inline banner** appears under the topbar in the app's webview area:
   > *example.com wants to use your camera and microphone.* &nbsp; **[Allow]** **[Block]**
2. The decision is **per-app**, stored in the app's config, and persists across sessions.
3. Two small icons (camera and microphone) live in the topbar to the right of the back/reload buttons, scoped to the **currently active app**:
   - **Hidden** — the app has never been granted or blocked for this device (state = `Ask`) and is not capturing.
   - **Outlined gray** — granted, not currently in use.
   - **Filled accent color** — granted and actively capturing.
   - **Slashed gray** — blocked.
4. **Clicking an allowed (gray or colored) icon** → flips the permission to `Block` immediately. No confirmation dialog. Any in-flight capture is left untouched (WebKit handles that); future `getUserMedia` calls will be denied.
5. **Clicking a slashed icon** → flips the permission to `Ask` and reloads the active webview, giving the page an opportunity to re-request and trigger a fresh banner.

The icon-click toggle is intentionally simple and reversible: a wrong click on the "allowed" icon is undone by clicking the slashed icon (which reloads). No modal confirmations.

## Architecture

### Data model — `src-tauri/src/config/models.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PermissionState {
    #[default]
    Ask,
    Allow,
    Block,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppPermissions {
    #[serde(default)]
    pub camera: PermissionState,
    #[serde(default)]
    pub microphone: PermissionState,
}

pub struct AppConfig {
    // ...existing fields...
    #[serde(default)]
    pub permissions: AppPermissions,
}
```

`Ask` is the default. Both the new field and the inner fields use `#[serde(default)]`, so existing TOML configs deserialize without migration.

A small helper enum `MediaKind { Camera, Microphone }` is shared across commands and serialized as `"camera"` / `"microphone"` on the wire.

### Runtime state — `src-tauri/src/state.rs`

```rust
pub pending_permission_requests:
    Mutex<HashMap<String, PendingMediaRequest>>,
pub active_captures:
    Mutex<HashMap<String, ActiveCaptures>>,
```

- `PendingMediaRequest` holds the WebKit `UserMediaPermissionRequest` plus flags indicating which kinds were requested.
- `ActiveCaptures` is `{ camera: bool, microphone: bool }`, updated by WebKit `notify::*-capture-state` signals.

Keyed by `app_id` (not `(app_id, kind)`) because WebKit issues one request that can ask for camera, microphone, or both.

### Backend flow — `src-tauri/src/commands/webviews.rs`

Inside the existing `#[cfg(target_os = "linux")] { ... with_webview(...) }` block (mirrors how ITP / cookie policy is set today), additionally connect three WebKit signals:

1. **`permission-request`** — downcast to `UserMediaPermissionRequest`. If not media, return `false` (let WebKit default). Otherwise call `handle_media_request`.
2. **`notify::camera-capture-state`** — update `active_captures[app_id].camera`, emit `media-capture-changed`.
3. **`notify::microphone-capture-state`** — same for microphone.

`handle_media_request(app_handle, app_id, request, wants_video, wants_audio)`:

1. Look up the app's stored `AppPermissions` (clone out of the spaces lock).
2. Resolve each requested kind:
   - All `Allow` → `request.allow()` immediately.
   - Any `Block` (for a requested kind) → `request.deny()` immediately.
   - Otherwise (at least one `Ask`) → store the request in `pending_permission_requests` and emit `media-permission-request` with `{ app_id, camera: wants_video, microphone: wants_audio }`.

A pending request persists across app-switches; the banner is keyed to the active app.

### New Tauri commands

- `respond_media_permission(app_id, camera: Option<PermissionState>, microphone: Option<PermissionState>)`
  - Used by the banner. Persists the decisions to `AppConfig`, then resolves the pending request: `allow()` only if every requested kind ended up `Allow`, otherwise `deny()`. Removes the entry from `pending_permission_requests`.
- `set_app_permission(app_id, kind: MediaKind, state: PermissionState)`
  - Used by clicking the topbar icons. Persists to config, emits `media-permission-changed`. Never touches `pending_permission_requests`.
- `get_app_permissions(app_id) -> AppPermissions` — convenience getter (the frontend can also read it from the `spaces` store; this is a fallback for direct reads).

### Events emitted to the frontend

- `media-permission-request` → `{ app_id, camera, microphone }`
- `media-permission-changed` → `{ app_id, permissions: AppPermissions }` (also triggers a `spaces` reload so the store stays in sync)
- `media-capture-changed` → `{ app_id, kind, active }`

## Frontend

### Store — `src/lib/stores/permissions.ts` (new)

- `pendingRequest: Writable<{ appId, camera, microphone } | null>`
- `activeCaptures: Writable<Map<string, { camera: boolean, microphone: boolean }>>`
- Subscribers wired in `App.svelte` to the three Tauri events above.

App-configured permissions (the `AppPermissions` field on `AppConfig`) are read from the existing `spaces` store — no duplication.

### API wrappers — `src/lib/api.ts`

```ts
respondMediaPermission(appId, camera?: PermissionState, microphone?: PermissionState)
setAppPermission(appId, kind: 'camera'|'microphone', state: PermissionState)
```

### Components

**`PermissionBanner.svelte` (new)** — mounted in `App.svelte`, positioned absolutely under the topbar over the app's webview area. Visible iff `pendingRequest` is non-null **and** its `appId` matches the active app. Shows the origin of the active app's URL, lists which devices are being requested, and renders **Allow** / **Block** buttons. Both call `respondMediaPermission` with the chosen state for each requested kind and clear `pendingRequest`. No dismiss-without-choosing path.

**`PermissionIcons.svelte` (new)** — placed in `TopBar.svelte` between the existing nav buttons and the right edge. Iterates camera + microphone; for the active app, picks the icon variant from the state table above. Click handlers:
- Allowed icon → `setAppPermission(activeApp, kind, 'block')`.
- Slashed icon → `setAppPermission(activeApp, kind, 'ask')` then `webviewReload()`.

A tooltip on each icon describes the current state and the click action.

**`TopBar.svelte`** — add `<PermissionIcons />` next to `nav-buttons`.

**`App.svelte`** — mount `<PermissionBanner />`; register listeners for the three new events on mount.

### Edge cases

- **App-switch with pending request:** banner hides (it's keyed to active app), request stays pending in Rust; switching back re-shows the banner.
- **App slept while capturing:** WebKit tears down the webview, which fires the capture-state notify with inactive; the icon transitions filled → outlined naturally. Capture entries for destroyed webviews are cleaned up in `close_app` / `sleep_app_inner`.
- **Combined camera+mic request:** one banner, one Allow click covers both. If the user wants to allow only one, that requires the two-icon path (click Block, then site re-requests just the other) — acceptable for v1.
- **Multiple requests stacked:** the second request supersedes the first (the first is denied automatically) to avoid queueing complexity. In practice apps rarely re-request before the user responds.

## Testing

- Rust unit tests for the permission-resolution logic in `handle_media_request` (allow / deny / ask matrix).
- Manual verification:
  1. Open a video-conferencing site (e.g., Whereby, Jitsi) in a new app. Banner appears. Click Allow → camera + mic activate, icons turn filled accent color.
  2. Click the camera icon → it becomes slashed; future calls denied.
  3. Click the slashed camera icon → page reloads, banner re-appears.
  4. Restart the app → previously allowed/blocked state persists, no banner on reload.
  5. Verify config TOML on disk contains `[permissions]` block with `camera = "allow"` etc.

## Out of scope

- macOS and Windows backends (different APIs entirely; not blocked by this design — the WebKit code is already in a `#[cfg(target_os = "linux")]` block).
- Other permission kinds (geolocation, notifications, clipboard, MIDI). Same WebKit `permission-request` signal can be extended later.
- Per-origin granularity within an app.
- Allow-once / temporary grants.
- Bulk permission management UI (Settings → Permissions list). The TOML is editable for now.
