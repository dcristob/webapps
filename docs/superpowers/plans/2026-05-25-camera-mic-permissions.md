# Camera & Microphone Permissions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gate `getUserMedia` requests behind a user-facing Allow/Block prompt, persist the decision per app, and show always-visible camera/mic state icons in the topbar that toggle the permission.

**Architecture:** WebKitGTK exposes the `permission-request` signal on each `WebView`. We connect to it from inside the existing `with_webview` Linux block in `commands/webviews.rs`. When a media request arrives, we consult the per-app `AppPermissions` stored in `AppConfig`. If a decision exists, we resolve synchronously; otherwise we stash the `PermissionRequest`, emit an event to the frontend, and resolve it later from a new Tauri command. "Currently capturing" state comes from `notify::camera-capture-state` / `notify::microphone-capture-state` signals on the WebView.

The Allow/Block banner is rendered by JS injected into the app webview (same pattern as the existing `LINK_INTERCEPTOR_JS`). This avoids resizing layouts and keeps the banner anchored to the page content. The topbar icons live in the existing topbar webview (`TopBar.svelte`).

**Tech Stack:** Rust + Tauri v2 (with `unstable` feature), WebKitGTK 2.0, Svelte 5 + TypeScript.

**Spec:** `docs/superpowers/specs/2026-05-25-camera-mic-permissions-design.md`

---

## Task 1: Add permission types to config models

**Files:**
- Modify: `src-tauri/src/config/models.rs`
- Test: `src-tauri/src/config/models.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/src/config/models.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_defaults_permissions_to_ask() {
        let toml = r#"
id = "abc"
name = "Test"
url = "https://example.com"
icon = "auto"
isolation_override = false
"#;
        let app: AppConfig = toml::from_str(toml).expect("parse");
        assert_eq!(app.permissions.camera, PermissionState::Ask);
        assert_eq!(app.permissions.microphone, PermissionState::Ask);
    }

    #[test]
    fn app_config_roundtrip_with_permissions() {
        let app = AppConfig {
            id: "abc".to_string(),
            name: "Test".to_string(),
            url: "https://example.com".to_string(),
            icon: "auto".to_string(),
            isolation_override: false,
            permissions: AppPermissions {
                camera: PermissionState::Allow,
                microphone: PermissionState::Block,
            },
        };
        let s = toml::to_string(&app).expect("serialize");
        let back: AppConfig = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.permissions.camera, PermissionState::Allow);
        assert_eq!(back.permissions.microphone, PermissionState::Block);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd src-tauri && cargo test --lib config::models::tests
```

Expected: FAIL — `PermissionState`, `AppPermissions`, and `permissions` field do not exist.

- [ ] **Step 3: Add the types**

In `src-tauri/src/config/models.rs`, add above the existing `AppConfig` struct:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PermissionState {
    #[default]
    Ask,
    Allow,
    Block,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Camera,
    Microphone,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPermissions {
    #[serde(default)]
    pub camera: PermissionState,
    #[serde(default)]
    pub microphone: PermissionState,
}
```

Then add the new field to `AppConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub icon: String,
    pub isolation_override: bool,
    #[serde(default)]
    pub permissions: AppPermissions,
}
```

- [ ] **Step 4: Fix the `add_app` constructor**

In `src-tauri/src/commands/apps.rs`, update the `AppConfig { ... }` literal inside `add_app` (lines 10–16) to set the new field:

```rust
let app = AppConfig {
    id: Uuid::new_v4().to_string(),
    name,
    url,
    icon: icon.unwrap_or_else(|| "auto".to_string()),
    isolation_override: false,
    permissions: AppPermissions::default(),
};
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cd src-tauri && cargo test --lib config::models::tests
```

Expected: PASS (both tests).

- [ ] **Step 6: Verify the full build still compiles**

```bash
cd src-tauri && cargo build
```

Expected: builds successfully.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/config/models.rs src-tauri/src/commands/apps.rs
git commit -m "feat: add per-app camera/microphone permission state"
```

---

## Task 2: Extend AppState with pending requests and capture tracking

**Files:**
- Modify: `src-tauri/src/state.rs`

This task only changes the struct definition and its initialization in `lib.rs`. There is nothing to test in isolation — coverage comes from later tasks. Keep the commit small.

- [ ] **Step 1: Add fields to `AppState`**

In `src-tauri/src/state.rs`, add to the `AppState` struct:

```rust
use webkit2gtk::UserMediaPermissionRequest;

pub struct AppState {
    // ...existing fields stay as-is...

    /// Map of app_id -> pending WebKit media permission request waiting for the user.
    /// `wants_camera` / `wants_microphone` flags say which devices the page asked for.
    #[cfg(target_os = "linux")]
    pub pending_media_requests: Mutex<HashMap<String, PendingMediaRequest>>,

    /// Map of app_id -> currently-capturing flags, updated by WebKit capture-state signals.
    pub active_captures: Mutex<HashMap<String, ActiveCaptures>>,
}

#[cfg(target_os = "linux")]
pub struct PendingMediaRequest {
    pub request: UserMediaPermissionRequest,
    pub wants_camera: bool,
    pub wants_microphone: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ActiveCaptures {
    pub camera: bool,
    pub microphone: bool,
}

// SAFETY: WebKit objects are not Send by default, but we only ever touch the
// PermissionRequest from the GTK main thread (where the signal is emitted and
// where webview.eval / state operations run via Tauri's main-thread tasks).
// Mutex requires Send; Tauri State requires Sync. We mark PendingMediaRequest
// Send + Sync because access is serialized to the GTK main thread by virtue
// of being called only from Tauri commands and signal handlers.
#[cfg(target_os = "linux")]
unsafe impl Send for PendingMediaRequest {}
#[cfg(target_os = "linux")]
unsafe impl Sync for PendingMediaRequest {}
```

- [ ] **Step 2: Initialize the new fields in `lib.rs`**

In `src-tauri/src/lib.rs`, find the `.manage(AppState { ... })` call (around line 68) and add the fields. The full block becomes:

```rust
.manage(AppState {
    global_config: Mutex::new(global_config),
    spaces: Mutex::new(spaces),
    active_space_id: Mutex::new("general".to_string()),
    active_app_id: Mutex::new(None),
    webview_labels: Mutex::new(HashMap::new()),
    context_menu_target: Mutex::new(None),
    space_context_menu_target: Mutex::new(None),
    last_active: Mutex::new(HashMap::new()),
    slept_apps: Mutex::new(std::collections::HashSet::new()),
    #[cfg(target_os = "linux")]
    pending_media_requests: Mutex::new(HashMap::new()),
    active_captures: Mutex::new(HashMap::new()),
})
```

- [ ] **Step 3: Build to verify**

```bash
cd src-tauri && cargo build
```

Expected: builds successfully.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat: add runtime state for pending media permissions and captures"
```

---

## Task 3: Add `set_app_permission` Tauri command

**Files:**
- Create: `src-tauri/src/commands/permissions.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register command)

This command flips a stored permission state from the topbar icon click. It does NOT touch any pending request.

- [ ] **Step 1: Create the new commands module**

Create `src-tauri/src/commands/permissions.rs`:

```rust
use tauri::{AppHandle, Emitter, State};

use crate::config::models::{AppPermissions, MediaKind, PermissionState};
use crate::config::storage;
use crate::state::AppState;

#[tauri::command]
pub fn set_app_permission(
    app_handle: AppHandle,
    space_id: String,
    app_id: String,
    kind: MediaKind,
    state_value: PermissionState,
    state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter_mut()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    let app = space
        .apps
        .iter_mut()
        .find(|a| a.id == app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;

    match kind {
        MediaKind::Camera => app.permissions.camera = state_value,
        MediaKind::Microphone => app.permissions.microphone = state_value,
    }

    let perms = app.permissions.clone();
    storage::save_space(space).map_err(|e| e.to_string())?;

    let _ = app_handle.emit(
        "media-permission-changed",
        serde_json::json!({
            "app_id": app_id,
            "permissions": perms,
        }),
    );

    Ok(perms)
}

#[tauri::command]
pub fn get_app_permissions(
    space_id: String,
    app_id: String,
    state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    let app = space
        .apps
        .iter()
        .find(|a| a.id == app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;
    Ok(app.permissions.clone())
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod permissions;
```

(Add it alphabetically among the existing `pub mod` declarations.)

- [ ] **Step 3: Wire the commands into the Tauri builder**

In `src-tauri/src/lib.rs`, find the `.invoke_handler(tauri::generate_handler![ ... ])` block (around line 264) and add:

```rust
commands::permissions::set_app_permission,
commands::permissions::get_app_permissions,
```

at the end of the list (before the closing `]`).

- [ ] **Step 4: Add a unit test for `set_app_permission`'s persistence logic**

Append to `src-tauri/src/commands/permissions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::{AppConfig, IsolationMode, SpaceConfig, SpaceInfo};

    fn sample_space() -> SpaceConfig {
        SpaceConfig {
            space: SpaceInfo {
                id: "s1".to_string(),
                name: "S1".to_string(),
                icon: "folder".to_string(),
                color: "#000000".to_string(),
                isolation: IsolationMode::Shared,
            },
            apps: vec![AppConfig {
                id: "a1".to_string(),
                name: "A1".to_string(),
                url: "https://example.com".to_string(),
                icon: "auto".to_string(),
                isolation_override: false,
                permissions: AppPermissions::default(),
            }],
        }
    }

    #[test]
    fn mutating_camera_does_not_touch_microphone() {
        let mut space = sample_space();
        let app = space.apps.iter_mut().find(|a| a.id == "a1").unwrap();
        app.permissions.camera = PermissionState::Allow;
        assert_eq!(app.permissions.camera, PermissionState::Allow);
        assert_eq!(app.permissions.microphone, PermissionState::Ask);
    }
}
```

(We test the data-mutation logic directly because exercising the full Tauri command needs a running `AppHandle`. Integration test of the command happens via manual verification in Task 12.)

- [ ] **Step 5: Run tests**

```bash
cd src-tauri && cargo test --lib commands::permissions
```

Expected: PASS.

- [ ] **Step 6: Build**

```bash
cd src-tauri && cargo build
```

Expected: builds successfully.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/permissions.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add set_app_permission and get_app_permissions commands"
```

---

## Task 4: Add `respond_media_permission` command and resolution logic

**Files:**
- Modify: `src-tauri/src/commands/permissions.rs`
- Modify: `src-tauri/src/lib.rs`

This command is called by the in-page banner when the user clicks Allow or Block. It persists the decisions to config and resolves the pending WebKit request.

- [ ] **Step 1: Add the resolution helper and command**

Append to `src-tauri/src/commands/permissions.rs`:

```rust
#[tauri::command]
#[cfg(target_os = "linux")]
pub fn respond_media_permission(
    app_handle: AppHandle,
    space_id: String,
    app_id: String,
    camera: Option<PermissionState>,
    microphone: Option<PermissionState>,
    state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    use webkit2gtk::PermissionRequestExt;

    // 1. Persist the user's decisions.
    let perms = {
        let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
        let space = spaces
            .iter_mut()
            .find(|s| s.space.id == space_id)
            .ok_or_else(|| format!("Space '{}' not found", space_id))?;
        let app = space
            .apps
            .iter_mut()
            .find(|a| a.id == app_id)
            .ok_or_else(|| format!("App '{}' not found", app_id))?;
        if let Some(c) = camera {
            app.permissions.camera = c;
        }
        if let Some(m) = microphone {
            app.permissions.microphone = m;
        }
        let perms = app.permissions.clone();
        storage::save_space(space).map_err(|e| e.to_string())?;
        perms
    };

    // 2. Resolve the pending WebKit request, if any.
    let pending = {
        let mut pending_map = state.pending_media_requests.lock().map_err(|e| e.to_string())?;
        pending_map.remove(&app_id)
    };

    if let Some(p) = pending {
        let camera_ok = !p.wants_camera || perms.camera == PermissionState::Allow;
        let mic_ok = !p.wants_microphone || perms.microphone == PermissionState::Allow;
        if camera_ok && mic_ok {
            p.request.allow();
        } else {
            p.request.deny();
        }
    }

    let _ = app_handle.emit(
        "media-permission-changed",
        serde_json::json!({
            "app_id": app_id,
            "permissions": perms,
        }),
    );

    Ok(perms)
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
pub fn respond_media_permission(
    _app_handle: AppHandle,
    _space_id: String,
    _app_id: String,
    _camera: Option<PermissionState>,
    _microphone: Option<PermissionState>,
    _state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    Err("Media permissions are only supported on Linux for now".to_string())
}
```

- [ ] **Step 2: Register the new command in `lib.rs`**

In `src-tauri/src/lib.rs`, in the `invoke_handler` list, add:

```rust
commands::permissions::respond_media_permission,
```

- [ ] **Step 3: Build**

```bash
cd src-tauri && cargo build
```

Expected: builds successfully.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/permissions.rs src-tauri/src/lib.rs
git commit -m "feat: add respond_media_permission command to resolve pending requests"
```

---

## Task 5: Wire WebKit `permission-request` signal in `open_app`

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`

Hook into the existing `with_webview` Linux block to handle media permission requests.

- [ ] **Step 1: Add the signal handler helper**

Add this private function near the top of `src-tauri/src/commands/webviews.rs` (after `LINK_INTERCEPTOR_JS`):

```rust
#[cfg(target_os = "linux")]
fn handle_media_permission_request(
    app_handle: &AppHandle,
    space_id: &str,
    app_id: &str,
    request: webkit2gtk::UserMediaPermissionRequest,
) {
    use webkit2gtk::{PermissionRequestExt, UserMediaPermissionRequestExt};

    let wants_video = request.is_for_video_device();
    let wants_audio = request.is_for_audio_device();
    if !wants_video && !wants_audio {
        // Not a request for video/audio capture (e.g. display capture). Deny by default.
        request.deny();
        return;
    }

    // Look up stored permissions for this app.
    let state = app_handle.state::<crate::state::AppState>();
    let (camera_state, microphone_state) = {
        let spaces = match state.spaces.lock() {
            Ok(g) => g,
            Err(_) => {
                request.deny();
                return;
            }
        };
        let app = spaces
            .iter()
            .find(|s| s.space.id == space_id)
            .and_then(|s| s.apps.iter().find(|a| a.id == app_id));
        match app {
            Some(a) => (a.permissions.camera, a.permissions.microphone),
            None => {
                request.deny();
                return;
            }
        }
    };

    use crate::config::models::PermissionState;

    let camera_decision = if wants_video { Some(camera_state) } else { None };
    let mic_decision = if wants_audio { Some(microphone_state) } else { None };

    let any_block = matches!(camera_decision, Some(PermissionState::Block))
        || matches!(mic_decision, Some(PermissionState::Block));
    let all_allow = camera_decision.map_or(true, |s| s == PermissionState::Allow)
        && mic_decision.map_or(true, |s| s == PermissionState::Allow);
    let any_ask = matches!(camera_decision, Some(PermissionState::Ask))
        || matches!(mic_decision, Some(PermissionState::Ask));

    if any_block {
        request.deny();
        return;
    }
    if all_allow && !any_ask {
        request.allow();
        return;
    }

    // At least one kind is Ask → stash and prompt the user.
    {
        let mut pending = match state.pending_media_requests.lock() {
            Ok(g) => g,
            Err(_) => {
                request.deny();
                return;
            }
        };
        // If a pending request already exists for this app, deny the new one to
        // avoid queueing complexity.
        if pending.contains_key(app_id) {
            request.deny();
            return;
        }
        pending.insert(
            app_id.to_string(),
            crate::state::PendingMediaRequest {
                request: request.clone(),
                wants_camera: wants_video,
                wants_microphone: wants_audio,
            },
        );
    }

    let _ = app_handle.emit(
        "media-permission-request",
        serde_json::json!({
            "space_id": space_id,
            "app_id": app_id,
            "camera": wants_video,
            "microphone": wants_audio,
        }),
    );
}
```

- [ ] **Step 2: Connect the signal inside `open_app`**

In `src-tauri/src/commands/webviews.rs`, locate the existing Linux `with_webview` block (around lines 196–210). Extend it so it both does the existing ITP/cookie config AND connects the permission signal. Replace the whole `#[cfg(target_os = "linux")] { if let Some(webview) = ... }` block with:

```rust
#[cfg(target_os = "linux")]
{
    let app_handle_for_perm = app_handle.clone();
    let space_id_for_perm = space_id.clone();
    let app_id_for_perm = app_clone.id.clone();
    if let Some(webview) = app_handle.get_webview(&label) {
        let _ = webview.with_webview(move |platform_webview| {
            use webkit2gtk::{
                CookieManagerExt, WebViewExt, WebsiteDataManagerExt,
            };
            let wk_webview = platform_webview.inner();
            if let Some(data_manager) = wk_webview.website_data_manager() {
                data_manager.set_itp_enabled(false);
                if let Some(cookie_manager) = data_manager.cookie_manager() {
                    cookie_manager.set_accept_policy(webkit2gtk::CookieAcceptPolicy::Always);
                }
            }

            // Media permission requests
            let app_handle_inner = app_handle_for_perm.clone();
            let space_id_inner = space_id_for_perm.clone();
            let app_id_inner = app_id_for_perm.clone();
            wk_webview.connect_permission_request(move |_wv, request| {
                if let Ok(media_req) = request.clone().downcast::<webkit2gtk::UserMediaPermissionRequest>() {
                    handle_media_permission_request(
                        &app_handle_inner,
                        &space_id_inner,
                        &app_id_inner,
                        media_req,
                    );
                    return true; // we handled it (possibly async)
                }
                false
            });
        });
    }
}
```

(Note: `space_id` here refers to the `space_id` parameter of `open_app`. It is owned `String`, so `.clone()` works directly.)

- [ ] **Step 3: Build**

```bash
cd src-tauri && cargo build
```

Expected: builds successfully. If `connect_permission_request` is reported missing, ensure `webkit2gtk::WebViewExt` is in scope (it already is via the existing `use` line).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs
git commit -m "feat: intercept WebKit media permission requests and route to user"
```

---

## Task 6: Add capture-state signal handlers

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`

Connect `notify::camera-capture-state` and `notify::microphone-capture-state` so the frontend can show "in use" coloring.

- [ ] **Step 1: Extend the Linux `with_webview` block**

In `src-tauri/src/commands/webviews.rs`, inside the same Linux block you edited in Task 5 (the body of the `.with_webview(...)` closure), append after the `wk_webview.connect_permission_request(...)` block:

```rust
// Capture-state notifications
let app_handle_cap = app_handle_for_perm.clone();
let app_id_cap = app_id_for_perm.clone();
wk_webview.connect_camera_capture_state_notify(move |wv| {
    use webkit2gtk::WebViewExt;
    let active = matches!(
        wv.camera_capture_state(),
        webkit2gtk::MediaCaptureState::Active
    );
    update_capture_state(&app_handle_cap, &app_id_cap, "camera", active);
});

let app_handle_cap2 = app_handle_for_perm.clone();
let app_id_cap2 = app_id_for_perm.clone();
wk_webview.connect_microphone_capture_state_notify(move |wv| {
    use webkit2gtk::WebViewExt;
    let active = matches!(
        wv.microphone_capture_state(),
        webkit2gtk::MediaCaptureState::Active
    );
    update_capture_state(&app_handle_cap2, &app_id_cap2, "microphone", active);
});
```

- [ ] **Step 2: Add the helper function**

Add this private function near `handle_media_permission_request`:

```rust
#[cfg(target_os = "linux")]
fn update_capture_state(app_handle: &AppHandle, app_id: &str, kind: &str, active: bool) {
    let state = app_handle.state::<crate::state::AppState>();
    if let Ok(mut captures) = state.active_captures.lock() {
        let entry = captures.entry(app_id.to_string()).or_default();
        match kind {
            "camera" => entry.camera = active,
            "microphone" => entry.microphone = active,
            _ => {}
        }
    }
    let _ = app_handle.emit(
        "media-capture-changed",
        serde_json::json!({
            "app_id": app_id,
            "kind": kind,
            "active": active,
        }),
    );
}
```

- [ ] **Step 3: Build**

```bash
cd src-tauri && cargo build
```

Expected: builds successfully. If `connect_camera_capture_state_notify` is missing in your `webkit2gtk` version, check the crate docs at https://docs.rs/webkit2gtk/ for the exact method name (it may be `connect_camera_capture_state_notify` or `connect_property_camera_capture_state_notify` depending on version). Adjust accordingly.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs
git commit -m "feat: emit capture-state changes from WebKit signals"
```

---

## Task 7: Clean up state on close/sleep

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`

When an app is closed or slept, drop any pending request (denying it) and clear capture state. Otherwise the icon may stay "in use" after the webview is destroyed.

- [ ] **Step 1: Add a cleanup helper**

Add this private function in `src-tauri/src/commands/webviews.rs`:

```rust
fn cleanup_media_state(app_handle: &AppHandle, app_id: &str, state: &AppState) {
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::PermissionRequestExt;
        if let Ok(mut pending) = state.pending_media_requests.lock() {
            if let Some(p) = pending.remove(app_id) {
                p.request.deny();
            }
        }
    }
    if let Ok(mut captures) = state.active_captures.lock() {
        captures.remove(app_id);
    }
    let _ = app_handle.emit(
        "media-capture-changed",
        serde_json::json!({
            "app_id": app_id,
            "kind": "camera",
            "active": false,
        }),
    );
    let _ = app_handle.emit(
        "media-capture-changed",
        serde_json::json!({
            "app_id": app_id,
            "kind": "microphone",
            "active": false,
        }),
    );
}
```

- [ ] **Step 2: Call cleanup from `close_app`**

Find `close_app` (around line 272 in the original file). At the end of the function body, just before the final `Ok(())`, add:

```rust
cleanup_media_state(&app_handle, &app_id, &state);
```

- [ ] **Step 3: Call cleanup from `sleep_app_inner`**

Find `sleep_app_inner` (around line 353). At the end of the function body, just before the final `Ok(())`, add:

```rust
cleanup_media_state(app_handle, app_id, state);
```

(Note the difference in `&` vs `*` — `sleep_app_inner` already takes references.)

- [ ] **Step 4: Build**

```bash
cd src-tauri && cargo build
```

Expected: builds successfully.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/webviews.rs
git commit -m "fix: clean up pending permission and capture state on close/sleep"
```

---

## Task 8: Frontend types and API wrappers

**Files:**
- Modify: `src/lib/types/index.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add types**

In `src/lib/types/index.ts`, add at the bottom:

```ts
export type PermissionState = "ask" | "allow" | "block";
export type MediaKind = "camera" | "microphone";

export interface AppPermissions {
  camera: PermissionState;
  microphone: PermissionState;
}
```

Then update the existing `AppConfig` interface to include:

```ts
permissions: AppPermissions;
```

(If `AppConfig` doesn't currently model every field returned by Rust, just ensure the new field is added.)

- [ ] **Step 2: Add API wrappers**

In `src/lib/api.ts`, add at the bottom:

```ts
import type { AppPermissions, MediaKind, PermissionState } from "./types";

export async function setAppPermission(
  spaceId: string,
  appId: string,
  kind: MediaKind,
  stateValue: PermissionState,
): Promise<AppPermissions> {
  return invoke("set_app_permission", {
    spaceId,
    appId,
    kind,
    stateValue,
  });
}

export async function getAppPermissions(
  spaceId: string,
  appId: string,
): Promise<AppPermissions> {
  return invoke("get_app_permissions", { spaceId, appId });
}

export async function respondMediaPermission(
  spaceId: string,
  appId: string,
  camera: PermissionState | null,
  microphone: PermissionState | null,
): Promise<AppPermissions> {
  return invoke("respond_media_permission", {
    spaceId,
    appId,
    camera,
    microphone,
  });
}
```

Note: Tauri command argument names use snake_case in Rust but are exposed as camelCase to JS, with one exception — when a Rust arg shadows the `state: State<...>`, we renamed it to `state_value`, which Tauri exposes as `stateValue`. That's what the wrapper sends.

- [ ] **Step 3: Verify the frontend type-checks**

```bash
npm run check 2>/dev/null || npx svelte-check
```

Expected: no new errors (existing errors unrelated to this change may remain).

- [ ] **Step 4: Commit**

```bash
git add src/lib/types/index.ts src/lib/api.ts
git commit -m "feat: add frontend types and API wrappers for media permissions"
```

---

## Task 9: Permissions store

**Files:**
- Create: `src/lib/stores/permissions.ts`

- [ ] **Step 1: Create the store**

Create `src/lib/stores/permissions.ts`:

```ts
import { writable } from "svelte/store";

export interface PendingRequest {
  spaceId: string;
  appId: string;
  camera: boolean;
  microphone: boolean;
}

export interface CaptureState {
  camera: boolean;
  microphone: boolean;
}

export const pendingRequest = writable<PendingRequest | null>(null);
export const activeCaptures = writable<Map<string, CaptureState>>(new Map());

export function setCapture(appId: string, kind: "camera" | "microphone", active: boolean) {
  activeCaptures.update((m) => {
    const next = new Map(m);
    const cur = next.get(appId) ?? { camera: false, microphone: false };
    const updated = { ...cur, [kind]: active };
    next.set(appId, updated);
    return next;
  });
}

export function clearCaptures(appId: string) {
  activeCaptures.update((m) => {
    const next = new Map(m);
    next.delete(appId);
    return next;
  });
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/stores/permissions.ts
git commit -m "feat: add Svelte store for media permissions and captures"
```

---

## Task 10: Wire backend events into the store

**Files:**
- Modify: `src/App.svelte`

The sidebar webview is the main shell (`App.svelte`). We register listeners there so events are received once globally. (The topbar webview is a separate process; we'll have it listen to the same events when we add the icons in Task 12.)

- [ ] **Step 1: Add listeners in `App.svelte`**

In `src/App.svelte`, inside `<script lang="ts">`, add:

```ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { pendingRequest, setCapture, clearCaptures } from "./lib/stores/permissions";
import { loadSpaces } from "./lib/stores/spaces";
```

Then, inside the existing `onMount` (create one if it doesn't exist), append:

```ts
const unlistenReq = await listen<{
  space_id: string;
  app_id: string;
  camera: boolean;
  microphone: boolean;
}>("media-permission-request", (event) => {
  pendingRequest.set({
    spaceId: event.payload.space_id,
    appId: event.payload.app_id,
    camera: event.payload.camera,
    microphone: event.payload.microphone,
  });
});

const unlistenCap = await listen<{
  app_id: string;
  kind: "camera" | "microphone";
  active: boolean;
}>("media-capture-changed", (event) => {
  if (!event.payload.active) {
    setCapture(event.payload.app_id, event.payload.kind, false);
  } else {
    setCapture(event.payload.app_id, event.payload.kind, true);
  }
});

const unlistenChanged = await listen<{ app_id: string }>(
  "media-permission-changed",
  async () => {
    await loadSpaces();
  },
);
```

In the existing `onDestroy` (create one if needed), add:

```ts
unlistenReq?.();
unlistenCap?.();
unlistenChanged?.();
```

(Declare `let unlistenReq: UnlistenFn | null = null;` etc. at the top of the script.)

- [ ] **Step 2: Build the frontend to verify**

```bash
npm run check 2>/dev/null || npx svelte-check
```

Expected: no new errors.

- [ ] **Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat: subscribe sidebar shell to media permission events"
```

---

## Task 11: Permission banner component (in-app, injected via JS)

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`
- Modify: `src/App.svelte`

We render the banner by injecting JS into the active app webview when `pendingRequest` is set for that app. This keeps the banner anchored to the page area without webview-resizing complexity.

- [ ] **Step 1: Add a Tauri command to evaluate JS in an app's webview**

Append to `src-tauri/src/commands/webviews.rs`:

```rust
#[tauri::command]
pub fn eval_in_app(app_handle: AppHandle, app_id: String, script: String, state: State<'_, AppState>) -> Result<(), String> {
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    let label = labels.get(&app_id).ok_or("Webview not found")?;
    let webview = app_handle.get_webview(label).ok_or("Webview not found")?;
    webview.eval(&script).map_err(|e| e.to_string())
}
```

Register it in `src-tauri/src/lib.rs` invoke_handler:

```rust
commands::webviews::eval_in_app,
```

- [ ] **Step 2: Add API wrapper**

In `src/lib/api.ts`, add:

```ts
export async function evalInApp(appId: string, script: string): Promise<void> {
  return invoke("eval_in_app", { appId, script });
}
```

- [ ] **Step 3: Add banner injection logic in `App.svelte`**

In `src/App.svelte`, add the JS template literal and reactive injection:

```ts
import { evalInApp, respondMediaPermission } from "./lib/api";

const BANNER_JS = (kinds: string, allowFn: string, blockFn: string) => `
(function() {
  var EXISTING = document.getElementById('__webapps_perm_banner');
  if (EXISTING) EXISTING.remove();

  var bar = document.createElement('div');
  bar.id = '__webapps_perm_banner';
  bar.style.cssText = [
    'position:fixed','top:0','left:0','right:0','z-index:2147483647',
    'background:#222','color:#fff','padding:10px 16px',
    'font-family:-apple-system,BlinkMacSystemFont,sans-serif','font-size:14px',
    'display:flex','align-items:center','gap:12px',
    'box-shadow:0 2px 8px rgba(0,0,0,0.3)',
  ].join(';') + ';';
  bar.innerHTML =
    '<span style="flex:1">' + ${JSON.stringify(`This app wants to use your ${kinds}.`)} + '</span>' +
    '<button id="__webapps_perm_allow" style="background:#4a9eff;color:#fff;border:none;padding:6px 14px;border-radius:4px;cursor:pointer;font-size:14px">Allow</button>' +
    '<button id="__webapps_perm_block" style="background:#444;color:#fff;border:none;padding:6px 14px;border-radius:4px;cursor:pointer;font-size:14px">Block</button>';
  document.documentElement.appendChild(bar);

  document.getElementById('__webapps_perm_allow').addEventListener('click', function() {
    window.__TAURI_INTERNALS__.invoke('respond_media_permission', ${allowFn});
    bar.remove();
  });
  document.getElementById('__webapps_perm_block').addEventListener('click', function() {
    window.__TAURI_INTERNALS__.invoke('respond_media_permission', ${blockFn});
    bar.remove();
  });
})();
`;

const BANNER_REMOVE_JS = `
(function() {
  var b = document.getElementById('__webapps_perm_banner');
  if (b) b.remove();
})();
`;

import { onMount } from "svelte";
import { get } from "svelte/store";

$effect(() => {
  const req = $pendingRequest;
  if (!req) return;

  const kinds: string[] = [];
  if (req.camera) kinds.push("camera");
  if (req.microphone) kinds.push("microphone");
  const kindsText = kinds.join(" and ");

  const allowArgs = JSON.stringify({
    spaceId: req.spaceId,
    appId: req.appId,
    camera: req.camera ? "allow" : null,
    microphone: req.microphone ? "allow" : null,
  });
  const blockArgs = JSON.stringify({
    spaceId: req.spaceId,
    appId: req.appId,
    camera: req.camera ? "block" : null,
    microphone: req.microphone ? "block" : null,
  });

  evalInApp(req.appId, BANNER_JS(kindsText, allowArgs, blockArgs)).catch(() => {});
});
```

Also import `pendingRequest`:

```ts
import { pendingRequest } from "./lib/stores/permissions";
```

And listen for `media-permission-changed` to clear the local store:

In the `media-permission-changed` listener you added in Task 10, also do:

```ts
pendingRequest.set(null);
```

(Add at the top of the listener.)

- [ ] **Step 4: Build and verify**

```bash
cd src-tauri && cargo build
cd .. && npm run check 2>/dev/null || npx svelte-check
```

Expected: both succeed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/webviews.rs src-tauri/src/lib.rs src/lib/api.ts src/App.svelte
git commit -m "feat: inject in-app permission banner via JS eval"
```

---

## Task 12: Topbar permission icons

**Files:**
- Create: `src/lib/components/PermissionIcons.svelte`
- Modify: `src/lib/components/TopBar.svelte`

The topbar is a separate webview from the sidebar shell, so it needs to register its own event listeners for capture state and read permissions from `loadSpaces()`.

- [ ] **Step 1: Create `PermissionIcons.svelte`**

Create `src/lib/components/PermissionIcons.svelte`:

```svelte
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { activeCaptures, setCapture, clearCaptures } from "../stores/permissions";
  import { spaces, activeSpace } from "../stores/spaces";
  import { activeAppId } from "../stores/apps";
  import { setAppPermission } from "../api";
  import { webviewReload } from "../api";

  let unlistenCap: UnlistenFn | null = null;

  onMount(async () => {
    unlistenCap = await listen<{ app_id: string; kind: "camera" | "microphone"; active: boolean }>(
      "media-capture-changed",
      (event) => {
        setCapture(event.payload.app_id, event.payload.kind, event.payload.active);
      },
    );
  });

  onDestroy(() => {
    unlistenCap?.();
  });

  // Resolve current app's permissions from the spaces store
  const currentApp = $derived.by(() => {
    const sid = $activeAppId;
    if (!sid) return null;
    for (const sp of $spaces) {
      const app = sp.apps.find((a) => a.id === sid);
      if (app) return { app, spaceId: sp.space.id };
    }
    return null;
  });

  const cameraState = $derived(currentApp?.app.permissions?.camera ?? "ask");
  const micState = $derived(currentApp?.app.permissions?.microphone ?? "ask");

  const captures = $derived($activeCaptures.get($activeAppId ?? "") ?? { camera: false, microphone: false });

  function classFor(state: string, active: boolean): string {
    if (state === "block") return "icon slashed";
    if (state === "ask") return "icon hidden";
    return active ? "icon active" : "icon allowed";
  }

  async function onClick(kind: "camera" | "microphone", state: string) {
    const app = currentApp;
    if (!app) return;
    if (state === "block") {
      await setAppPermission(app.spaceId, app.app.id, kind, "ask");
      await webviewReload();
    } else if (state === "allow") {
      await setAppPermission(app.spaceId, app.app.id, kind, "block");
    }
  }
</script>

{#if currentApp}
  <div class="permission-icons">
    {#if cameraState !== "ask" || captures.camera}
      <button
        class={classFor(cameraState, captures.camera)}
        title={cameraState === "block" ? "Camera blocked. Click to allow." : (captures.camera ? "Camera in use. Click to block." : "Camera allowed. Click to block.")}
        onclick={() => onClick("camera", cameraState)}
        aria-label="Camera permission"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="2" y="6" width="14" height="12" rx="2" />
          <path d="M22 8l-6 4 6 4V8z" />
          {#if cameraState === "block"}
            <line x1="3" y1="3" x2="23" y2="21" stroke="currentColor" stroke-width="2" />
          {/if}
        </svg>
      </button>
    {/if}
    {#if micState !== "ask" || captures.microphone}
      <button
        class={classFor(micState, captures.microphone)}
        title={micState === "block" ? "Microphone blocked. Click to allow." : (captures.microphone ? "Microphone in use. Click to block." : "Microphone allowed. Click to block.")}
        onclick={() => onClick("microphone", micState)}
        aria-label="Microphone permission"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="9" y="2" width="6" height="12" rx="3" />
          <path d="M5 11a7 7 0 0 0 14 0" />
          <line x1="12" y1="18" x2="12" y2="22" />
          {#if micState === "block"}
            <line x1="3" y1="3" x2="23" y2="21" stroke="currentColor" stroke-width="2" />
          {/if}
        </svg>
      </button>
    {/if}
  </div>
{/if}

<style>
  .permission-icons {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .icon {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: none;
    background: transparent;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
  }
  .icon:hover {
    background: var(--bg-hover, #333);
  }
  .icon.allowed { color: var(--text-secondary, #888); }
  .icon.active  { color: #4a9eff; }
  .icon.slashed { color: var(--text-secondary, #666); }
  .icon.hidden  { display: none; }
</style>
```

- [ ] **Step 2: Mount it in the topbar**

In `src/lib/components/TopBar.svelte`, add the import:

```ts
import PermissionIcons from "./PermissionIcons.svelte";
```

Then in the markup, add `<PermissionIcons />` inside `.nav-buttons`, after the reload button:

```html
<div class="nav-buttons">
  <button class="nav-btn" onclick={() => webviewGoBack()} title="Go back">...</button>
  <button class="nav-btn" onclick={() => webviewReload()} title="Reload">...</button>
  <PermissionIcons />
</div>
```

- [ ] **Step 3: Verify**

```bash
npm run check 2>/dev/null || npx svelte-check
```

Expected: no new errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/PermissionIcons.svelte src/lib/components/TopBar.svelte
git commit -m "feat: add camera/mic permission icons to topbar"
```

---

## Task 13: Manual end-to-end verification

**Files:** None (manual verification).

UI/permission flows can't be reliably automated in this stack. Run through the spec scenarios manually.

- [ ] **Step 1: Start dev mode**

```bash
npm run tauri dev
```

Wait for the window to appear.

- [ ] **Step 2: Verify default state (clean config)**

- Add a new app pointing to a known camera/mic test site, e.g. `https://webrtc.github.io/samples/src/content/getusermedia/gum/`.
- Open the app. Click the page's "Open camera" button.
- **Expected:** Banner appears at the top of the page with two buttons: Allow and Block.

- [ ] **Step 3: Verify Allow path**

- Click **Allow**.
- **Expected:**
  - Camera feed starts in the page.
  - Camera icon appears in the topbar, filled accent color.
  - When the page stops the stream, the icon transitions to outlined gray.

- [ ] **Step 4: Verify config persistence**

- Stop dev mode (`Ctrl+C` in terminal).
- Open `~/.config/webapps/spaces/<space-id>.toml`.
- **Expected:** The app entry has a `[apps.permissions]` block with `camera = "allow"`.
- Restart dev mode. Reload the app. **Expected:** Banner does NOT appear; camera grants immediately.

- [ ] **Step 5: Verify Block toggle**

- Click the camera icon in the topbar.
- **Expected:** Icon becomes slashed gray. Trigger camera in the page again. **Expected:** Page reports permission denied; no banner.

- [ ] **Step 6: Verify slashed → re-ask**

- Click the slashed camera icon.
- **Expected:** Page reloads. Trigger camera again. **Expected:** Banner reappears.

- [ ] **Step 7: Verify mic + combined request**

- Repeat steps 2–6 with microphone using a site that requests both (e.g., `https://meet.jit.si`).
- **Expected:** Single banner mentioning both; one Allow covers both; two icons render in the topbar; both go filled when streaming.

- [ ] **Step 8: Verify cleanup on close**

- With camera active, remove the app from the sidebar (or close the active webview via the existing close path).
- **Expected:** Camera icon disappears from topbar; no stale "in use" state. Check no leftover entries by reopening the app.

- [ ] **Step 9: If any step fails**

Mark the failing step and debug before declaring done. Common failure points:
- `connect_permission_request` signature mismatch → check `webkit2gtk` crate docs.
- Capture-state signal name varies between versions → try `connect_property_camera_capture_state_notify`.
- Banner not appearing → check the active app webview's devtools console (right-click → Inspect) for the injected element `#__webapps_perm_banner`.
- Permission changes not persisting → check `~/.config/webapps/spaces/*.toml` after the click.

- [ ] **Step 10: Final commit (if any small fixes were needed)**

```bash
git add -A
git commit -m "fix: address issues found during manual verification"
```

(If no changes, skip.)

---

## Self-Review Notes

Spec coverage check:
- ✅ Per-app scope: `AppPermissions` lives on `AppConfig` (Task 1).
- ✅ Inline banner: in-page JS injection with Allow/Block (Task 11).
- ✅ Topbar icons with 4 states: `PermissionIcons.svelte` (Task 12).
- ✅ One-click toggle, no confirm: handled in `onClick` (Task 12).
- ✅ Persistence in app config TOML: `storage::save_space` (Tasks 3, 4).
- ✅ Block applies to future requests; in-flight capture left alone: WebKit handles this naturally.
- ✅ Slashed click → reset to Ask + reload: handled in `onClick` (Task 12).
- ✅ Capture-state events: Task 6.
- ✅ Cleanup on close/sleep: Task 7.

Placeholder scan: none.

Type consistency check: `PermissionState` / `MediaKind` / `AppPermissions` defined in Task 1 and used consistently in Tasks 3, 4, 8, 9, 11, 12.
