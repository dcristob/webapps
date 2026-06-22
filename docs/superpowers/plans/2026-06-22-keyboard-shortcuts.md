# Keyboard Shortcuts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add WebApps-focused keyboard shortcuts for cycling/jumping apps, toggling the sidebar, adding an app, sleeping the current app, and switching spaces via a searchable palette.

**Architecture:** Two listener surfaces route to one Rust command `handle_shortcut`: (1) a `keydown` listener injected into every app webview (capture-phase, `preventDefault` so the hosted app never sees the key — "shell wins" like Slack/Rambox); (2) a Svelte `keydown` listener in the sidebar/topbar webviews. `handle_shortcut` dispatches almost entirely to existing commands (`open_app`, `sleep_app_inner`, `show_dialog`); only sidebar-toggle geometry and the palette are new. No new Cargo dependency, no OS-level global-hotkey registration.

**Tech Stack:** Rust + Tauri v2 (unstable multi-webview); Svelte 5 + TypeScript; TOML config; WebKitGTK on Linux.

## Global Constraints

(Copied verbatim from the approved spec — every task's requirements include these.)

- **Scope:** shortcuts fire only when the WebApps window has keyboard focus. Do NOT add `tauri-plugin-global-shortcut` or any new Cargo dependency.
- **Bindings (fixed for v1):** `Ctrl+Tab`/`Ctrl+Shift+Tab` (cycle), `Ctrl+1`–`Ctrl+9` (jump), `Ctrl+B` (toggle sidebar), `Ctrl+N` (add app), `Ctrl+W` (sleep app), `Ctrl+Shift+S` (space palette). Do NOT use `Ctrl+Space` / `Ctrl+Shift+Space` — they are the ibus/fcitx IME hotkey on Linux and never reach JS.
- **`Ctrl+W` = reversible sleep** (`sleep_app_inner`), NOT destructive "remove app from space".
- **Escape is shell/dialog-only** — never inject an Escape handler into app webviews (hosted apps must keep their own Escape behavior). Existing dialog components and GTK context menus already handle Escape; only the palette adds its own (Task 9).
- **Shell wins:** the injected listener and the Svelte listener both call `e.preventDefault(); e.stopImmediatePropagation()` before invoking, so the hosted app / page never also reacts.
- **Maintainer platform:** Arch Linux / WebKitGTK. GTK-specific code is behind `#[cfg(target_os = "linux")]` as in the existing codebase.
- **Frontend verify command:** `npm run check` (svelte-check). There is no `npm run lint`.
- **Rust verify commands:** `cargo test` and `cargo build`, run from `src-tauri/`.

## Testing strategy

- **Rust pure logic** → real `cargo test` unit tests (Tasks 1–3).
- **Tauri commands, JS injection, Svelte components** → no headless webview harness exists, so verification is `cargo build` (Rust) / `npm run check` (TS) plus an explicit manual check in `npm run tauri dev`. Each such step says exactly what to look for.

---

## Task 1: Pure shortcut-index helpers (TDD)

**Files:**
- Create: `src-tauri/src/commands/shortcuts.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Test: `src-tauri/src/commands/shortcuts.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum CycleDir { Next, Prev }`; `pub fn cycle_index(current: usize, len: usize, dir: CycleDir) -> Option<usize>`; `pub fn jump_index(n: usize, len: usize) -> Option<usize>`; `pub fn neighbor_index(removed: usize, len_before: usize) -> Option<usize>`. Task 6 consumes these.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/commands/shortcuts.rs` with only the test module and the type/fn signatures as `todo!()` stubs:

```rust
//! Keyboard-shortcut dispatch: pure index helpers, the injected listener JS,
//! and the `handle_shortcut` command. Built up across tasks 1, 3, 4, 6.

/// Cycle direction for [`cycle_index`].
pub enum CycleDir {
    Next,
    Prev,
}

/// Wrapping index for cycling apps. `current` is the active app's index; returns
/// the next/previous index, wrapping. `None` if the list is empty.
pub fn cycle_index(_current: usize, _len: usize, _dir: CycleDir) -> Option<usize> {
    todo!()
}

/// 1-based positional jump → 0-based index. `None` for 0 or past the end. No wrap.
pub fn jump_index(_n: usize, _len: usize) -> Option<usize> {
    todo!()
}

/// After removing the app at `removed` from a list of `len_before`, which
/// surviving index to activate? Prefers the app that was after (it slides into
/// `removed`'s slot); else the new last; else `None` (was the only app). No wrap.
pub fn neighbor_index(_removed: usize, _len_before: usize) -> Option<usize> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_next_wraps() {
        assert_eq!(cycle_index(0, 3, CycleDir::Next), Some(1));
        assert_eq!(cycle_index(2, 3, CycleDir::Next), Some(0));
    }

    #[test]
    fn cycle_prev_wraps() {
        assert_eq!(cycle_index(1, 3, CycleDir::Prev), Some(0));
        assert_eq!(cycle_index(0, 3, CycleDir::Prev), Some(2));
    }

    #[test]
    fn cycle_empty_returns_none() {
        assert_eq!(cycle_index(0, 0, CycleDir::Next), None);
        assert_eq!(cycle_index(0, 0, CycleDir::Prev), None);
    }

    #[test]
    fn jump_one_based_no_wrap() {
        assert_eq!(jump_index(1, 3), Some(0));
        assert_eq!(jump_index(3, 3), Some(2));
        assert_eq!(jump_index(4, 3), None);
        assert_eq!(jump_index(0, 3), None);
    }

    #[test]
    fn neighbor_prefers_after_then_before() {
        assert_eq!(neighbor_index(1, 3), Some(1)); // middle: after slides in
        assert_eq!(neighbor_index(2, 3), Some(1)); // last: new last
        assert_eq!(neighbor_index(0, 3), Some(0)); // first of many: after slides in
    }

    #[test]
    fn neighbor_only_app_returns_none() {
        assert_eq!(neighbor_index(0, 1), None);
        assert_eq!(neighbor_index(0, 0), None);
    }
}
```

Register the module in `src-tauri/src/commands/mod.rs` (keep alphabetical order):

```rust
pub mod apps;
pub mod dialog;
pub mod favicon;
pub mod permissions;
pub mod shortcuts;
pub mod spaces;
pub mod webviews;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml cycle_index`
Expected: the `cycle_index` tests PANIC with `not yet implemented` (the `todo!()` stubs).

- [ ] **Step 3: Implement the helpers**

Replace the three `todo!()` bodies in `src-tauri/src/commands/shortcuts.rs`:

```rust
pub fn cycle_index(current: usize, len: usize, dir: CycleDir) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match dir {
        CycleDir::Next => (current + 1) % len,
        CycleDir::Prev => (current + len - 1) % len,
    })
}

pub fn jump_index(n: usize, len: usize) -> Option<usize> {
    if n == 0 || n > len {
        None
    } else {
        Some(n - 1)
    }
}

pub fn neighbor_index(removed: usize, len_before: usize) -> Option<usize> {
    if len_before <= 1 {
        return None;
    }
    if removed + 1 < len_before {
        Some(removed)
    } else {
        Some(removed - 1)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — all 6 new shortcut tests pass, plus all pre-existing tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/shortcuts.rs src-tauri/src/commands/mod.rs
git commit -m "feat(shortcuts): add pure cycle/jump/neighbor index helpers"
```

---

## Task 2: Persisted `sidebar_visible` state

**Files:**
- Modify: `src-tauri/src/config/models.rs` (add field + default + tests)
- Modify: `src-tauri/src/state.rs` (add `AppState` field)
- Modify: `src-tauri/src/lib.rs` (init from config)

**Interfaces:**
- Consumes: nothing.
- Produces: `GlobalConfig.general.sidebar_visible: bool` (serde default `true`); `AppState.sidebar_visible: Mutex<bool>`. Tasks 4 and 6 consume these.

- [ ] **Step 1: Write the failing tests**

In `src-tauri/src/config/models.rs`, append two tests to the existing `#[cfg(test)] mod tests` block (after `app_config_roundtrip_with_permissions`):

```rust
    #[test]
    fn global_config_defaults_sidebar_visible_true() {
        let cfg = GlobalConfig::default();
        assert!(cfg.general.sidebar_visible);
    }

    #[test]
    fn global_config_sidebar_visible_defaults_when_missing() {
        // A config written before this field existed must default to visible.
        let toml = r#"
[general]
sidebar_width = 100
theme = "dark"
"#;
        let cfg: GlobalConfig = toml::from_str(toml).expect("parse");
        assert!(cfg.general.sidebar_visible);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml global_config_sidebar`
Expected: FAIL to compile — `sidebar_visible` is not a field on `GeneralSettings`.

- [ ] **Step 3: Add the field + default**

In `src-tauri/src/config/models.rs`, add the field to `GeneralSettings` (next to `space_order`) and a default fn (next to `default_sleep_timeout`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    pub sidebar_width: u32,
    pub theme: String,
    /// Minutes of inactivity before a background app's webview is destroyed to save memory.
    /// 0 = disabled.
    #[serde(default = "default_sleep_timeout")]
    pub sleep_timeout_mins: u32,
    #[serde(default)]
    pub space_order: Vec<String>,
    /// Whether the sidebar webview is shown. Toggled by the Ctrl+B shortcut.
    #[serde(default = "default_sidebar_visible")]
    pub sidebar_visible: bool,
}

fn default_sleep_timeout() -> u32 {
    15
}

fn default_sidebar_visible() -> bool {
    true
}
```

And update the `Default for GlobalConfig` impl so the constructed `GeneralSettings` includes the new field:

```rust
impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            general: GeneralSettings {
                sidebar_width: 100,
                theme: "dark".to_string(),
                sleep_timeout_mins: 15,
                space_order: vec![],
                sidebar_visible: true,
            },
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml global_config`
Expected: PASS — both new tests pass.

- [ ] **Step 5: Add the `AppState` field**

In `src-tauri/src/state.rs`, add the field to `AppState` (after `active_app_id`):

```rust
    pub active_app_id: Mutex<Option<String>>,
    /// Whether the sidebar webview is currently shown (toggled by Ctrl+B).
    pub sidebar_visible: Mutex<bool>,
    pub webview_labels: Mutex<HashMap<String, String>>,
```

- [ ] **Step 6: Init the field from config at startup**

In `src-tauri/src/lib.rs`, read the value alongside `sidebar_width` and pass it into `AppState`. The top of `run()` currently has:

```rust
    let global_config = storage::load_global_config().unwrap_or_default();
    let sidebar_width = global_config.general.sidebar_width;
```

Change to:

```rust
    let global_config = storage::load_global_config().unwrap_or_default();
    let sidebar_width = global_config.general.sidebar_width;
    let sidebar_visible = global_config.general.sidebar_visible;
```

And in the `.manage(AppState { ... })` initializer, add the field (place it right after `active_app_id`):

```rust
            active_app_id: Mutex::new(None),
            sidebar_visible: Mutex::new(sidebar_visible),
            webview_labels: Mutex::new(HashMap::new()),
```

- [ ] **Step 7: Build to verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds with no errors.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/config/models.rs src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat(config): add persisted sidebar_visible setting"
```

---

## Task 3: `SHORTCUT_LISTENER_JS` builder (TDD)

**Files:**
- Modify: `src-tauri/src/commands/shortcuts.rs` (append builder + test)

**Interfaces:**
- Consumes: nothing new.
- Produces: `pub fn build_shortcut_listener_js() -> &'static str`. Tasks 6 (dispatch) and 7 (injection) consume it.

- [ ] **Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` block in `src-tauri/src/commands/shortcuts.rs`:

```rust
    #[test]
    fn shortcut_listener_js_contains_all_actions() {
        let js = build_shortcut_listener_js();
        for needle in [
            "cycle-next",
            "cycle-prev",
            "toggle-sidebar",
            "add-app",
            "sleep-app",
            "space-switcher",
            "handle_shortcut",
        ] {
            assert!(js.contains(needle), "shortcut listener JS missing {needle}");
        }
        // Capture phase + shell-wins behavior.
        assert!(js.contains("addEventListener(\"keydown\""));
        assert!(js.contains("e.preventDefault()"));
        assert!(js.contains("e.stopImmediatePropagation()"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml shortcut_listener_js`
Expected: compile error — `build_shortcut_listener_js` is not defined.

- [ ] **Step 3: Add the builder**

Append to `src-tauri/src/commands/shortcuts.rs` (above the `#[cfg(test)]` block):

```rust
/// JS injected into every app webview. Listens for our shortcut bindings in the
/// capture phase (so the hosted app never sees them — the shell wins, matching
/// Slack/Rambox), then forwards the matched action to `handle_shortcut`.
///
/// The key→action table MIRRORS the one in `src/lib/shortcuts.ts`. Both map onto
/// the same closed set of action strings — keep them in sync when editing.
pub fn build_shortcut_listener_js() -> &'static str {
    r#"
(function() {
  if (window.__webapps_shortcut_listener) return;
  window.__webapps_shortcut_listener = true;

  // (ctrl, shift, keyLower) -> action
  var TABLE = {
    "true|false|tab": "cycle-next",
    "true|true|tab": "cycle-prev",
    "true|false|b": "toggle-sidebar",
    "true|false|n": "add-app",
    "true|false|w": "sleep-app",
    "true|true|s": "space-switcher"
  };

  function actionFor(e) {
    var keyLower = (e.key || "").toLowerCase();
    // Ctrl+1..9 (no shift, no alt, no meta)
    if (e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey
        && keyLower.length === 1 && keyLower >= "1" && keyLower <= "9") {
      return "jump-" + keyLower;
    }
    if (!e.ctrlKey || e.metaKey || e.altKey) return null;
    var k = "true|" + (e.shiftKey ? "true" : "false") + "|" + keyLower;
    return TABLE[k] || null;
  }

  document.addEventListener("keydown", function(e) {
    var action = actionFor(e);
    if (!action) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (window.__TAURI_INTERNALS__) {
      try { window.__TAURI_INTERNALS__.invoke("handle_shortcut", { action: action }); }
      catch (err) { /* ignore */ }
    }
  }, true);
})();
"#
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS — all tests including the new one.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/shortcuts.rs
git commit -m "feat(shortcuts): add injected app-webview keydown listener builder"
```

---

## Task 4: Sidebar toggle + layout reposition helper

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs` (add helper + command; wire into `switch_to_app` + `ensure_app_open`)
- Modify: `src-tauri/src/lib.rs` (register `toggle_sidebar`)

**Interfaces:**
- Consumes: `AppState.sidebar_visible` and `GlobalConfig.general.sidebar_width` (Task 2); `storage::save_global_config`.
- Produces: `pub fn reposition_app_webviews(app_handle: &AppHandle, state: &AppState) -> Result<(), String>`; `pub fn toggle_sidebar_inner(app_handle: &AppHandle, state: &AppState) -> Result<(), String>`; command `toggle_sidebar`. Task 6's `handle_shortcut` consumes `toggle_sidebar_inner`.

- [ ] **Step 1: Add the reposition helper**

In `src-tauri/src/commands/webviews.rs`, add this function (e.g. right after the existing `get_active_app` command). `LogicalPosition`, `LogicalSize`, `Manager`, and `TOPBAR_HEIGHT` are already imported/defined in this file.

```rust
/// Reposition/resize the ACTIVE app webview to match current sidebar visibility
/// and sidebar width. Called from `toggle_sidebar`, `switch_to_app`, and
/// `ensure_app_open` so the layout never desyncs when the sidebar is toggled.
///
/// Hidden app webviews are irrelevant (they are `.hide()`-n elsewhere); only the
/// active one needs explicit geometry.
pub fn reposition_app_webviews(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    let (visible, sidebar_width) = {
        let visible = *state.sidebar_visible.lock().map_err(|e| e.to_string())?;
        let cfg = state.global_config.lock().map_err(|e| e.to_string())?;
        (visible, cfg.general.sidebar_width)
    };

    let active_id = state.active_app_id.lock().map_err(|e| e.to_string())?.clone();
    let Some(active_id) = active_id else { return Ok(()); };

    let label = {
        let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
        labels.get(&active_id).cloned()
    };
    let Some(label) = label else { return Ok(()); };
    let Some(webview) = app_handle.get_webview(&label) else { return Ok(()); };

    let window = app_handle.get_window("main").ok_or("Main window not found")?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let logical_w = size.width as f64 / scale;
    let logical_h = size.height as f64 / scale;

    let x = if visible { sidebar_width as f64 } else { 0.0 };
    let w = if visible { logical_w - sidebar_width as f64 } else { logical_w };
    webview
        .set_position(LogicalPosition::new(x, TOPBAR_HEIGHT))
        .map_err(|e| e.to_string())?;
    webview
        .set_size(LogicalSize::new(w, logical_h - TOPBAR_HEIGHT))
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: Add `toggle_sidebar_inner` + the `toggle_sidebar` command**

Append to `src-tauri/src/commands/webviews.rs`:

```rust
/// Flip sidebar visibility, persist it, hide/show the sidebar webview, and
/// resize the active app webview to fill the new area.
pub fn toggle_sidebar_inner(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    let new_visible = {
        let mut visible = state.sidebar_visible.lock().map_err(|e| e.to_string())?;
        *visible = !*visible;
        *visible
    };

    // Persist (non-fatal: in-memory state is already flipped).
    {
        let cfg = state.global_config.lock().map_err(|e| e.to_string())?;
        if let Err(e) = crate::config::storage::save_global_config(&cfg) {
            eprintln!("failed to persist sidebar_visible: {e}");
        }
    }

    if let Some(sidebar) = app_handle.get_webview("sidebar") {
        if new_visible {
            let _ = sidebar.show();
        } else {
            let _ = sidebar.hide();
        }
    }

    reposition_app_webviews(app_handle, state)?;
    Ok(())
}

#[tauri::command]
pub fn toggle_sidebar(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    toggle_sidebar_inner(&app_handle, &state)
}
```

- [ ] **Step 3: Wire `reposition_app_webviews` into `switch_to_app`**

`switch_to_app` currently holds the `webview_labels` (and `active_app_id`, `last_active`) `Mutex` guards until it returns. `reposition_app_webviews` also locks `webview_labels`, and `std::sync::Mutex` is NOT reentrant — so calling it while those guards live would **deadlock**. Restructure `switch_to_app` to scope each lock in its own block so all guards are dropped before the reposition call. Replace the entire current body of `switch_to_app` with:

```rust
pub fn switch_to_app(app_handle: AppHandle, _space_id: String, app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    // Hide all app webviews, then show the target. Scope the labels guard so it
    // is dropped before reposition_app_webviews re-locks webview_labels (Mutex is
    // not reentrant — holding it across the reposition call would deadlock).
    {
        let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
        for (_, label) in labels.iter() {
            if let Some(webview) = app_handle.get_webview(label) {
                let _ = webview.hide();
            }
        }
        if let Some(label) = labels.get(&app_id) {
            if let Some(webview) = app_handle.get_webview(label) {
                webview.show().map_err(|e| e.to_string())?;
            }
        }
    }

    {
        let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
        *active_app = Some(app_id.clone());
        let _ = app_handle.emit("active-app-changed", Some(app_id.clone()));
    }

    {
        let mut last_active = state.last_active.lock().map_err(|e| e.to_string())?;
        last_active.insert(app_id, Instant::now());
    }

    // Now no AppState locks are held: safe to reposition (which locks again).
    reposition_app_webviews(&app_handle, &state)?;
    Ok(())
}
```

Behavior is unchanged (same hides/show, same state updates, same emit) except the locks are released earlier and the just-shown webview is resized to match current sidebar visibility.

- [ ] **Step 4: Use current visibility for the initial `add_child` position in `ensure_app_open`**

In `ensure_app_open`, the sidebar offset currently always uses `sidebar_width`. Find:

```rust
    let sidebar_width = {
        let config = state.global_config.lock().map_err(|e| e.to_string())?;
        config.general.sidebar_width
    };
```

Replace with a read of both fields:

```rust
    let (sidebar_width, sidebar_visible) = {
        let config = state.global_config.lock().map_err(|e| e.to_string())?;
        (config.general.sidebar_width, *state.sidebar_visible.lock().map_err(|e| e.to_string())?)
    };
    let sidebar_x = if sidebar_visible { sidebar_width as f64 } else { 0.0 };
```

And update the `window.add_child(...)` call a few dozen lines below — change the position/size to honor `sidebar_x`:

```rust
    window.add_child(
        webview_builder,
        LogicalPosition::new(sidebar_x, TOPBAR_HEIGHT),
        LogicalSize::new(logical_width - sidebar_x, logical_height - TOPBAR_HEIGHT),
    ).map_err(|e| e.to_string())?;
```

- [ ] **Step 5: Register the `toggle_sidebar` command**

In `src-tauri/src/lib.rs`, add to the `invoke_handler!` list (e.g. right after `commands::webviews::show_app_context_menu`):

```rust
            commands::webviews::show_app_context_menu,
            commands::webviews::toggle_sidebar,
```

- [ ] **Step 6: Build to verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds with no errors.
If the build reports that `Webview` has no `set_position` / `set_size` methods on this Tauri version: stop and report — the fallback is the GTK widget `set_size_request` path on Linux; do NOT silently guess. (These methods exist on `tauri::Webview` in current Tauri v2 releases.)

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/webviews.rs src-tauri/src/lib.rs
git commit -m "feat(shortcuts): add toggle_sidebar + app-webview reposition helper"
```

---

## Task 5: Space-switcher + focus commands

**Files:**
- Modify: `src-tauri/src/commands/dialog.rs` (add `open_space_switcher` + `focus_active_app`)
- Modify: `src-tauri/src/lib.rs` (register both)

**Interfaces:**
- Consumes: `AppState.active_app_id`, `AppState.webview_labels`.
- Produces: command `open_space_switcher(app_handle)`; command `focus_active_app(app_handle, state)`. Task 6 (`handle_shortcut`) and Task 9 (palette component) consume these.

- [ ] **Step 1: Add `open_space_switcher`**

In `src-tauri/src/commands/dialog.rs`, add palette-specific size constants near the top (next to the existing `DIALOG_WIDTH`/`DIALOG_HEIGHT`) and the new command (after `close_dialog`). The command mirrors `show_dialog`'s centering + Linux transient/modal logic at a smaller size and reuses the `"dialog"` label so the one-dialog rule still holds:

```rust
const PALETTE_WIDTH: f64 = 420.0;
const PALETTE_HEIGHT: f64 = 360.0;

#[tauri::command]
pub fn open_space_switcher(app_handle: AppHandle) -> Result<(), String> {
    // Honor the one-dialog rule: if any dialog is already open, just focus it.
    if let Some(existing) = app_handle.get_webview_window("dialog") {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let url = "index.html?dialog=space-switcher";

    let main_window = app_handle.get_window("main").ok_or("Main window not found")?;
    let win_pos = main_window.outer_position().map_err(|e| e.to_string())?;
    let win_size = main_window.outer_size().map_err(|e| e.to_string())?;
    let scale = main_window.scale_factor().map_err(|e| e.to_string())?;

    let wlw = win_size.width as f64 / scale;
    let wlh = win_size.height as f64 / scale;
    let wlx = win_pos.x as f64 / scale;
    let wly = win_pos.y as f64 / scale;

    let x = wlx + (wlw - PALETTE_WIDTH) / 2.0;
    let y = wly + (wlh - PALETTE_HEIGHT) / 2.0;

    let dialog = WebviewWindowBuilder::new(
        &app_handle,
        "dialog",
        WebviewUrl::App(url.into()),
    )
    .title("Switch Space")
    .inner_size(PALETTE_WIDTH, PALETTE_HEIGHT)
    .position(x, y)
    .resizable(false)
    .decorations(false)
    .build()
    .map_err(|e| e.to_string())?;

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        use gtk::prelude::*;
        let parent_gtk = main_window.gtk_window().map_err(|e| e.to_string())?;
        let dialog_gtk = dialog.as_ref().window().gtk_window().map_err(|e| e.to_string())?;
        dialog_gtk.set_transient_for(Some(&parent_gtk));
        dialog_gtk.set_modal(true);
    }

    Ok(())
}
```

- [ ] **Step 2: Add `focus_active_app`**

Append to `src-tauri/src/commands/dialog.rs`. This needs `State`, so extend the file's existing `use tauri::{...}` import to include `State`, and add the `AppState` import. The current top import is:

```rust
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
```

Change it to:

```rust
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use crate::state::AppState;
```

Then add the command:

```rust
/// Return keyboard focus to the active app's webview. Used after a dialog
/// (e.g. the space switcher) closes so typing resumes in the app, not the shell.
#[tauri::command]
pub fn focus_active_app(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active_id = state.active_app_id.lock().map_err(|e| e.to_string())?.clone();
    if let Some(id) = active_id {
        let label = {
            let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
            labels.get(&id).cloned()
        };
        if let Some(label) = label {
            if let Some(webview) = app_handle.get_webview(&label) {
                let _ = webview.set_focus();
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Register both commands**

In `src-tauri/src/lib.rs`, add to `invoke_handler!` (after `commands::dialog::close_dialog`):

```rust
            commands::dialog::show_dialog,
            commands::dialog::close_dialog,
            commands::dialog::open_space_switcher,
            commands::dialog::focus_active_app,
```

- [ ] **Step 4: Build to verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds with no errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/dialog.rs src-tauri/src/lib.rs
git commit -m "feat(shortcuts): add space-switcher + focus-active-app commands"
```

---

## Task 6: `handle_shortcut` dispatch command

**Files:**
- Modify: `src-tauri/src/commands/shortcuts.rs` (add `handle_shortcut` + `active_space_apps` helper)
- Modify: `src-tauri/src/lib.rs` (register `handle_shortcut`)

**Interfaces:**
- Consumes: `cycle_index` / `jump_index` / `neighbor_index` / `CycleDir` (Task 1); `commands::webviews::open_app`, `sleep_app_inner`, `toggle_sidebar_inner` (Task 4); `commands::dialog::show_dialog`, `open_space_switcher` (Task 5).
- Produces: command `handle_shortcut(action: String)`. Tasks 7 (injected JS) and 8 (shell listener) invoke it.

- [ ] **Step 1: Add the read helper + dispatch command**

Append to `src-tauri/src/commands/shortcuts.rs`. Add the needed imports at the top of the file (after the `//!` doc comment) and the two functions above the `#[cfg(test)]` block:

```rust
use tauri::{AppHandle, State};

use crate::state::AppState;

/// Read a snapshot of the active space: `(space_id, ordered app_ids, active_app_id)`.
fn active_space_apps(state: &AppState) -> Result<(String, Vec<String>, Option<String>), String> {
    let active_space_id = state.active_space_id.lock().map_err(|e| e.to_string())?.clone();
    let active_app_id = state.active_app_id.lock().map_err(|e| e.to_string())?.clone();
    let app_ids = {
        let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
        let space = spaces
            .iter()
            .find(|s| s.space.id == active_space_id)
            .ok_or_else(|| format!("Space '{active_space_id}' not found"))?;
        space.apps.iter().map(|a| a.id.clone()).collect::<Vec<_>>()
    };
    Ok((active_space_id, app_ids, active_app_id))
}

/// Single dispatch point for every keyboard shortcut. The injected app-webview
/// listener and the Svelte shell listener both `invoke("handle_shortcut", { action })`.
#[tauri::command]
pub fn handle_shortcut(
    app_handle: AppHandle,
    action: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    match action.as_str() {
        "cycle-next" | "cycle-prev" => {
            let dir = if action == "cycle-next" {
                CycleDir::Next
            } else {
                CycleDir::Prev
            };
            let (space_id, app_ids, active) = active_space_apps(&state)?;
            if app_ids.is_empty() {
                return Ok(());
            }
            // Nothing active: next opens the first app, prev opens the last.
            let cur = active
                .as_deref()
                .and_then(|a| app_ids.iter().position(|id| id == a));
            let target_idx = match cur {
                Some(c) => cycle_index(c, app_ids.len(), dir),
                None => Some(if matches!(dir, CycleDir::Next) { 0 } else { app_ids.len() - 1 }),
            };
            if let Some(idx) = target_idx {
                let target = app_ids[idx].clone();
                crate::commands::webviews::open_app(app_handle, space_id, target, state)?;
            }
            Ok(())
        }
        s if s.starts_with("jump-") => {
            let n: usize = s["jump-".len()..]
                .parse()
                .map_err(|_| format!("bad jump action: {s}"))?;
            let (space_id, app_ids, _active) = active_space_apps(&state)?;
            if let Some(idx) = jump_index(n, app_ids.len()) {
                let target = app_ids[idx].clone();
                crate::commands::webviews::open_app(app_handle, space_id, target, state)?;
            }
            Ok(())
        }
        "toggle-sidebar" => {
            crate::commands::webviews::toggle_sidebar_inner(&app_handle, &state)?;
            Ok(())
        }
        "add-app" => {
            let space_id = state
                .active_space_id
                .lock()
                .map_err(|e| e.to_string())?
                .clone();
            crate::commands::dialog::show_dialog(
                app_handle,
                "add-app".to_string(),
                Some(space_id),
                None,
            )?;
            Ok(())
        }
        "sleep-app" => {
            let (space_id, app_ids, active) = active_space_apps(&state)?;
            let active_id = match active {
                Some(a) => a,
                None => return Ok(()),
            };
            let pos = match app_ids.iter().position(|id| *id == active_id) {
                Some(p) => p,
                None => return Ok(()),
            };
            // Reversible sleep, then switch to a neighbor (next else prev else none).
            crate::commands::webviews::sleep_app_inner(&app_handle, &active_id, &state)?;
            if let Some(nidx) = neighbor_index(pos, app_ids.len()) {
                let target = app_ids[nidx].clone();
                crate::commands::webviews::open_app(app_handle, space_id, target, state)?;
            }
            Ok(())
        }
        "space-switcher" => {
            crate::commands::dialog::open_space_switcher(app_handle)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
```

- [ ] **Step 2: Register the command**

In `src-tauri/src/lib.rs`, add to `invoke_handler!` (e.g. right after `commands::webviews::toggle_sidebar`):

```rust
            commands::webviews::toggle_sidebar,
            commands::shortcuts::handle_shortcut,
```

- [ ] **Step 3: Build and run all tests**

Run: `cargo build --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml`
Expected: builds clean; all tests pass (no new unit tests here — the pure logic was tested in Task 1; this is wiring).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/shortcuts.rs src-tauri/src/lib.rs
git commit -m "feat(shortcuts): add handle_shortcut dispatch command"
```

---

## Task 7: Inject the shortcut listener into app webviews

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs` (inject in `on_page_load`)

**Interfaces:**
- Consumes: `build_shortcut_listener_js()` (Task 3); `handle_shortcut` must be registered (Task 6).
- Produces: app webviews forward shortcut keystrokes to `handle_shortcut`.

- [ ] **Step 1: Inject the listener on page load**

In `ensure_app_open`, the `on_page_load` closure currently injects `MEDIA_GUARD_JS` on `Started` and the link interceptor on `Finished`. Add the shortcut listener to the `Started` branch. Find:

```rust
        .on_page_load(move |webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let _ = webview.eval(MEDIA_GUARD_JS);
                let _ = webview.eval(&window_open_override_js);
            }
```

Add the shortcut-listener injection right after the existing `Started` injections:

```rust
        .on_page_load(move |webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let _ = webview.eval(MEDIA_GUARD_JS);
                let _ = webview.eval(&window_open_override_js);
                let _ = webview.eval(crate::commands::shortcuts::build_shortcut_listener_js());
            }
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds with no errors.

- [ ] **Step 3: Manual verification**

Run: `npm run tauri dev`
In the running app: add 2–3 apps to a space, open one, then while it has focus press:
- `Ctrl+2` → switches to the 2nd app.
- `Ctrl+Tab` / `Ctrl+Shift+Tab` → cycles forward / backward.
- `Ctrl+1` (with only 3 apps, press `Ctrl+5`) → nothing happens (no-op, no wrap).
- `Ctrl+B` → sidebar hides and the app resizes to full width; `Ctrl+B` again → sidebar returns.
- `Ctrl+W` → current app sleeps (webview gone) and a neighbor becomes active.
- `Ctrl+N` → Add App dialog opens.
Confirm none of these also trigger the hosted app's own handler (the app should not react — shell wins).
If `Ctrl+B` does not resize the visible app on Linux, see the note in Task 4 Step 6 about the GTK fallback.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs
git commit -m "feat(shortcuts): inject keydown listener into app webviews"
```

---

## Task 8: Frontend shell listener (sidebar + topbar)

**Files:**
- Create: `src/lib/shortcuts.ts`
- Modify: `src/lib/api.ts` (add `handleShortcut`)
- Modify: `src/App.svelte` (install listener in non-dialog webviews)

**Interfaces:**
- Consumes: `invoke("handle_shortcut", { action })` (Task 6).
- Produces: `installShellShortcuts()` — used by `App.svelte`.

- [ ] **Step 1: Create `src/lib/shortcuts.ts`**

```ts
import { invoke } from "@tauri-apps/api/core";

// Shell-side binding table. MIRRORS the injected app-webview table in
// src-tauri/src/commands/shortcuts.rs (build_shortcut_listener_js). Both map
// onto the same closed set of action strings — keep them in sync.
//
// This listener covers the case where a shell webview (sidebar/topbar) has
// keyboard focus. Shortcuts typed inside a hosted app are caught by the
// injected JS instead.

function actionFor(e: KeyboardEvent): string | null {
  const keyLower = e.key.toLowerCase();

  // Ctrl+1..9 (no shift/alt/meta)
  if (
    e.ctrlKey &&
    !e.metaKey &&
    !e.altKey &&
    !e.shiftKey &&
    keyLower.length === 1 &&
    keyLower >= "1" &&
    keyLower <= "9"
  ) {
    return "jump-" + keyLower;
  }
  if (!e.ctrlKey || e.metaKey || e.altKey) return null;

  const k = `true|${e.shiftKey ? "true" : "false"}|${keyLower}`;
  switch (k) {
    case "true|false|tab":
      return "cycle-next";
    case "true|true|tab":
      return "cycle-prev";
    case "true|false|b":
      return "toggle-sidebar";
    case "true|false|n":
      return "add-app";
    case "true|false|w":
      return "sleep-app";
    case "true|true|s":
      return "space-switcher";
    default:
      return null;
  }
}

/** Attach the shell keydown listener (call in sidebar/topbar webviews).
 *  Returns a cleanup function. */
export function installShellShortcuts(): () => void {
  const handler = (e: KeyboardEvent) => {
    const action = actionFor(e);
    if (!action) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    void invoke("handle_shortcut", { action });
  };
  window.addEventListener("keydown", handler, true);
  return () => window.removeEventListener("keydown", handler, true);
}
```

- [ ] **Step 2: Add the `handleShortcut` wrapper**

In `src/lib/api.ts`, add (in the Dialog section near `showDialog`/`closeDialog`):

```ts
// Shortcuts
export async function handleShortcut(action: string): Promise<void> {
  return invoke("handle_shortcut", { action });
}
```

- [ ] **Step 3: Install the listener in `App.svelte`**

In `src/App.svelte`, add the import with the other lib imports:

```ts
  import { initTitleListener } from "./lib/stores/apps";
  import { installShellShortcuts } from "./lib/shortcuts";
```

Add a cleanup slot near the other `let unlisten…` declarations:

```ts
  let unlistenReq: UnlistenFn | null = null;
  let unlistenCap: UnlistenFn | null = null;
  let unlistenChanged: UnlistenFn | null = null;
  let unlistenCancelled: UnlistenFn | null = null;
  let cleanupShortcuts: (() => void) | null = null;
```

At the top of `onMount` (before the `if (!dialogMode && !mode)` branch), install for every non-dialog webview (sidebar + topbar):

```ts
  onMount(async () => {
    // Shell webviews (sidebar + topbar) get the keyboard-shortcut listener.
    // Dialog webviews manage their own input/Escape handling.
    if (!dialogMode) {
      cleanupShortcuts = installShellShortcuts();
    }

    // Sidebar mode: load spaces and init title listener
    if (!dialogMode && !mode) {
```

And clean up in `onDestroy`:

```ts
  onDestroy(() => {
    unlistenReq?.();
    unlistenCap?.();
    unlistenChanged?.();
    unlistenCancelled?.();
    cleanupShortcuts?.();
  });
```

- [ ] **Step 4: Typecheck**

Run: `npm run check`
Expected: PASS with no errors.

- [ ] **Step 5: Manual verification**

Run: `npm run tauri dev`
Click the "+" button in the sidebar (so the sidebar webview has focus), then press `Ctrl+N` → the Add App dialog opens. Press `Ctrl+B` → sidebar toggles. This confirms shortcuts fire from shell-webview focus, not just from app-webview focus.

- [ ] **Step 6: Commit**

```bash
git add src/lib/shortcuts.ts src/lib/api.ts src/App.svelte
git commit -m "feat(shortcuts): add shell keydown listener for sidebar/topbar"
```

---

## Task 9: Space-switcher palette component

**Files:**
- Create: `src/lib/components/SpaceSwitcherPalette.svelte`
- Modify: `src/lib/api.ts` (add `openSpaceSwitcher`, `focusActiveApp`)
- Modify: `src/App.svelte` (add the `space-switcher` dialog branch)

**Interfaces:**
- Consumes: `spaces` / `activeSpaceId` stores + `switchToSpace` (`stores/spaces`); `open_space_switcher` / `focus_active_app` commands (Task 5); `closeDialog`.
- Produces: the rendered palette shown when `?dialog=space-switcher`.

- [ ] **Step 1: Add the `focusActiveApp` wrapper**

In `src/lib/api.ts`, add to the Shortcuts section. (The palette is opened via `handle_shortcut` → the `open_space_switcher` command, so no JS `openSpaceSwitcher` wrapper is needed — only `focusActiveApp`, which the palette calls after switching.)

```ts
export async function focusActiveApp(): Promise<void> {
  return invoke("focus_active_app");
}
```

- [ ] **Step 2: Create `src/lib/components/SpaceSwitcherPalette.svelte`**

```svelte
<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { spaces, activeSpaceId, switchToSpace } from "../stores/spaces";
  import { focusActiveApp, closeDialog } from "../api";
  import { autofocus } from "../actions";

  let query = $state("");
  let selected = $state(0);

  // Filtered, case-insensitive substring match on space name. Empty query → all.
  let filtered = $derived(
    $spaces.filter((s) =>
      s.space.name.toLowerCase().includes(query.trim().toLowerCase())
    )
  );

  async function activate(spaceId: string) {
    await switchToSpace(spaceId);
    await emit("space-switched");
    await focusActiveApp();
    await closeDialog();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      void closeDialog();
      return;
    }
    if (filtered.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selected = (selected + 1) % filtered.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selected = (selected - 1 + filtered.length) % filtered.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const target = filtered[selected];
      if (target) void activate(target.space.id);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="palette">
  <input
    class="search"
    bind:value={query}
    oninput={() => (selected = 0)}
    placeholder="Switch space…"
    use:autofocus
  />
  <div class="list" role="listbox">
    {#each filtered as space, i (space.space.id)}
      <button
        class="row"
        class:selected={i === selected}
        class:active={$activeSpaceId === space.space.id}
        style="--space-color: {space.space.color}"
        onclick={() => activate(space.space.id)}
        role="option"
        title={space.space.name}
      >
        <span class="dot"></span>
        <span class="name">{space.space.name}</span>
        <span class="count">{space.apps.length} apps</span>
      </button>
    {:else}
      <div class="empty">No spaces match "{query}".</div>
    {/each}
  </div>
</div>

<style>
  .palette {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-primary, #1a1a1a);
    border: 1px solid var(--border-color, #444);
    border-radius: 8px;
    overflow: hidden;
  }
  .search {
    width: 100%;
    padding: 12px 14px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: none;
    border-bottom: 1px solid var(--border-color, #333);
    font-size: 14px;
    box-sizing: border-box;
    outline: none;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    background: transparent;
    color: var(--text-primary, #e0e0e0);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    font-size: 13px;
  }
  .row:hover,
  .row.selected {
    background: var(--bg-hover, #333);
  }
  .row.active .name {
    font-weight: 600;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--space-color, #4a9eff);
    flex-shrink: 0;
  }
  .name {
    flex: 1;
  }
  .count {
    color: var(--text-secondary, #888);
    font-size: 12px;
  }
  .empty {
    padding: 16px;
    color: var(--text-secondary, #888);
    font-size: 13px;
    text-align: center;
  }
</style>
```

- [ ] **Step 3: Mount the palette from `App.svelte`**

In `src/App.svelte`, add the import:

```ts
  import SpaceDialog from "./lib/components/SpaceDialog.svelte";
  import SpaceSwitcherPalette from "./lib/components/SpaceSwitcherPalette.svelte";
```

And add a dialog branch in the render block (after the `edit-space` branch, before `mode === "topbar"`):

```svelte
{:else if dialogMode === "edit-space"}
  <SpaceDialog
    mode="edit"
    spaceId={dialogSpaceId}
    initialName={dialogSpaceName}
    initialColor={dialogSpaceColor}
  />
{:else if dialogMode === "space-switcher"}
  <SpaceSwitcherPalette />
{:else if mode === "topbar"}
```

- [ ] **Step 4: Typecheck**

Run: `npm run check`
Expected: PASS with no errors.

- [ ] **Step 5: Manual verification**

Run: `npm run tauri dev`
Create at least 2 spaces (each with an app). Press `Ctrl+Shift+S` (from either an app or the sidebar) → the palette opens centered, search input focused. Type a few letters → list filters; `ArrowDown`/`ArrowUp` moves selection (wraps); `Enter` switches space and closes the palette, returning focus to the now-active app; `Escape` closes without switching; clicking a row switches.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/SpaceSwitcherPalette.svelte src/lib/api.ts src/App.svelte
git commit -m "feat(shortcuts): add searchable space-switcher palette"
```

---

## Final verification

After Task 9:

- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml` — all tests pass.
- [ ] Run `cargo build --manifest-path src-tauri/Cargo.toml` — clean build.
- [ ] Run `npm run check` — no type errors.
- [ ] End-to-end smoke test in `npm run tauri dev`: exercise every binding from both an app webview (focus an app) and the sidebar (click the "+" first). Confirm sidebar visibility persists across an app restart (it is saved to `~/.config/webapps/config.toml`).
- [ ] Capabilities check: custom commands registered via `invoke_handler!` are not gated by `capabilities/default.json` (app webviews already invoke `open_in_browser` etc. without entries). If a shortcut works from the sidebar but silently fails when an app has focus, that would indicate an IPC access issue — check `src-tauri/capabilities/default.json`. Expected: no change needed.
