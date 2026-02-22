# Edit App + Custom Icon Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow users to edit existing app entries (name, URL, icon) via a context menu "Edit" option that opens an edit dialog with a file picker for custom icons and a re-fetch favicon button.

**Architecture:** Add "Edit" to the native context menu → emit event with app data → Sidebar opens edit dialog → EditAppDialog component with pre-filled fields, native file picker (via tauri-plugin-dialog), and favicon re-fetch. Uses existing `edit_app` Rust command and `editApp()` TS wrapper.

**Tech Stack:** Tauri v2 (plugin-dialog for file picker), Svelte 5, TypeScript, Rust

---

### Task 1: Add tauri-plugin-dialog dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`
- Modify: `src-tauri/src/lib.rs:36` (plugin registration)
- Modify: `src-tauri/capabilities/default.json`

**Step 1: Add Rust dependency**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:
```toml
tauri-plugin-dialog = "2"
```

**Step 2: Add JS dependency**

Run: `npm install @tauri-apps/plugin-dialog`

**Step 3: Register the plugin in lib.rs**

In `src-tauri/src/lib.rs`, add `.plugin(tauri_plugin_dialog::init())` right after `.plugin(tauri_plugin_shell::init())`:
```rust
.plugin(tauri_plugin_shell::init())
.plugin(tauri_plugin_dialog::init())
```

**Step 4: Add dialog permissions to capabilities**

In `src-tauri/capabilities/default.json`, add to `permissions` array:
```json
"dialog:allow-open"
```

**Step 5: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles without errors.

**Step 6: Commit**

```bash
git add src-tauri/Cargo.toml package.json package-lock.json src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "chore: add tauri-plugin-dialog for file picker support"
```

---

### Task 2: Add "Edit" to context menu and emit app data

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs:193-210` (show_app_context_menu)
- Modify: `src-tauri/src/lib.rs:118-132` (on_menu_event)

**Step 1: Add "Edit" menu item to context menu**

In `src-tauri/src/commands/webviews.rs`, modify `show_app_context_menu` to add an Edit item before Remove:

```rust
#[tauri::command]
pub fn show_app_context_menu(app_handle: AppHandle, space_id: String, app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut target = state.context_menu_target.lock().map_err(|e| e.to_string())?;
        *target = Some((space_id, app_id));
    }

    let window = app_handle.get_window("main").ok_or("Main window not found")?;

    let edit_item = MenuItem::with_id(&app_handle, "ctx-edit-app", "Edit", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let remove_item = MenuItem::with_id(&app_handle, "ctx-remove-app", "Remove", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let menu = Menu::with_items(&app_handle, &[&edit_item, &remove_item])
        .map_err(|e| e.to_string())?;

    menu.popup(window).map_err(|e| e.to_string())?;

    Ok(())
}
```

**Step 2: Handle "ctx-edit-app" in on_menu_event**

In `src-tauri/src/lib.rs`, extend the `on_menu_event` closure. After the existing `ctx-remove-app` block, add handling for `ctx-edit-app`. This reads the app data from state and emits it:

```rust
.on_menu_event(|app_handle, event| {
    let state = app_handle.state::<AppState>();
    let target = {
        let mut guard = state.context_menu_target.lock().unwrap();
        guard.take()
    };

    if let Some((space_id, app_id)) = target {
        match event.id().as_ref() {
            "ctx-remove-app" => {
                let _ = app_handle.emit("context-menu-remove-app", serde_json::json!({
                    "space_id": space_id,
                    "app_id": app_id,
                }));
            }
            "ctx-edit-app" => {
                // Read app data from state to pass to the edit dialog
                let spaces = state.spaces.lock().unwrap();
                if let Some(space) = spaces.iter().find(|s| s.space.id == space_id) {
                    if let Some(app) = space.apps.iter().find(|a| a.id == app_id) {
                        let _ = app_handle.emit("context-menu-edit-app", serde_json::json!({
                            "space_id": space_id,
                            "app_id": app_id,
                            "name": app.name,
                            "url": app.url,
                            "icon": app.icon,
                        }));
                    }
                }
            }
            _ => {}
        }
    }
})
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs src-tauri/src/lib.rs
git commit -m "feat: add Edit item to app context menu and emit app data"
```

---

### Task 3: Extend show_dialog to accept extra URL params

**Files:**
- Modify: `src-tauri/src/commands/dialog.rs`
- Modify: `src/lib/api.ts:94-96` (showDialog wrapper)

**Step 1: Add params support to Rust show_dialog command**

In `src-tauri/src/commands/dialog.rs`, modify `show_dialog` to accept an optional `params` HashMap that gets appended to the URL:

```rust
use std::collections::HashMap;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const DIALOG_WIDTH: f64 = 450.0;
const DIALOG_HEIGHT: f64 = 300.0;

#[tauri::command]
pub fn show_dialog(app_handle: AppHandle, dialog_type: String, space_id: Option<String>, params: Option<HashMap<String, String>>) -> Result<(), String> {
    if let Some(existing) = app_handle.get_webview_window("dialog") {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut url = format!("index.html?dialog={}", dialog_type);
    if let Some(sid) = space_id {
        url.push_str(&format!("&spaceId={}", sid));
    }
    if let Some(extra) = params {
        for (key, value) in extra {
            url.push_str(&format!("&{}={}", key, urlencoding::encode(&value)));
        }
    }

    let title = match dialog_type.as_str() {
        "add-app" => "Add App",
        "edit-app" => "Edit App",
        "create-space" => "New Space",
        _ => "Dialog",
    };

    let main_window = app_handle.get_window("main").ok_or("Main window not found")?;
    let win_pos = main_window.outer_position().map_err(|e| e.to_string())?;
    let win_size = main_window.outer_size().map_err(|e| e.to_string())?;
    let scale = main_window.scale_factor().map_err(|e| e.to_string())?;

    let win_logical_w = win_size.width as f64 / scale;
    let win_logical_h = win_size.height as f64 / scale;
    let win_logical_x = win_pos.x as f64 / scale;
    let win_logical_y = win_pos.y as f64 / scale;

    let dialog_x = win_logical_x + (win_logical_w - DIALOG_WIDTH) / 2.0;
    let dialog_y = win_logical_y + (win_logical_h - DIALOG_HEIGHT) / 2.0;

    WebviewWindowBuilder::new(
        &app_handle,
        "dialog",
        WebviewUrl::App(url.into()),
    )
    .title(title)
    .inner_size(DIALOG_WIDTH, DIALOG_HEIGHT)
    .position(dialog_x, dialog_y)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}
```

Note: Add `urlencoding = "2"` to `src-tauri/Cargo.toml` dependencies.

**Step 2: Update TypeScript showDialog wrapper**

In `src/lib/api.ts`, update `showDialog` to accept an optional params map:

```typescript
export async function showDialog(dialogType: string, spaceId?: string, params?: Record<string, string>): Promise<void> {
  return invoke("show_dialog", { dialogType, spaceId: spaceId ?? null, params: params ?? null });
}
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/commands/dialog.rs src/lib/api.ts
git commit -m "feat: extend show_dialog to accept extra URL params"
```

---

### Task 4: Create EditAppDialog component

**Files:**
- Create: `src/lib/components/EditAppDialog.svelte`

**Step 1: Create the EditAppDialog component**

Create `src/lib/components/EditAppDialog.svelte`:

```svelte
<script lang="ts">
  import { emit } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { editApp, fetchSiteInfo, closeDialog } from "../api";

  let { spaceId, appId, initialName, initialUrl, initialIcon }: {
    spaceId: string;
    appId: string;
    initialName: string;
    initialUrl: string;
    initialIcon: string;
  } = $props();

  let name = $state(initialName);
  let url = $state(initialUrl);
  let icon = $state(initialIcon);
  let fetchingFavicon = $state(false);

  let iconPreviewSrc = $derived(
    icon && icon !== "auto"
      ? (icon.startsWith("/") ? convertFileSrc(icon) : icon)
      : null
  );

  async function handleChooseIcon() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "ico", "webp"] }],
    });
    if (selected) {
      icon = selected;
    }
  }

  async function handleRefetchFavicon() {
    if (!url.trim()) return;
    fetchingFavicon = true;
    try {
      const [, fetchedIcon] = await fetchSiteInfo(url.trim());
      icon = fetchedIcon;
    } catch {
      // Keep current icon on error
    }
    fetchingFavicon = false;
  }

  async function handleSave() {
    if (!name.trim()) return;
    await editApp(spaceId, appId, {
      name: name.trim(),
      url: url.trim() || undefined,
      icon: icon || undefined,
    });
    await emit("dialog-result", { type: "app-edited" });
    await closeDialog();
  }

  async function handleCancel() {
    await closeDialog();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") handleCancel();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="dialog">
  <h3>Edit App</h3>

  <label>
    Name
    <input bind:value={name} placeholder="App name" onkeydown={(e) => e.key === "Enter" && handleSave()} autofocus />
  </label>

  <label>
    URL
    <input bind:value={url} placeholder="https://example.com" onkeydown={(e) => e.key === "Enter" && handleSave()} />
  </label>

  <div class="icon-section">
    <span class="icon-label">Icon</span>
    <div class="icon-row">
      <div class="icon-preview">
        {#if iconPreviewSrc}
          <img src={iconPreviewSrc} alt="" width="32" height="32" />
        {:else}
          <span class="icon-placeholder">{name.charAt(0).toUpperCase()}</span>
        {/if}
      </div>
      <button class="icon-btn" onclick={handleChooseIcon}>Choose file...</button>
      <button class="icon-btn" onclick={handleRefetchFavicon} disabled={fetchingFavicon}>
        {fetchingFavicon ? "..." : "Re-fetch"}
      </button>
    </div>
  </div>

  <div class="actions">
    <button class="cancel" onclick={handleCancel}>Cancel</button>
    <button class="save" onclick={handleSave} disabled={!name.trim()}>Save</button>
  </div>
</div>

<style>
  .dialog {
    padding: 24px;
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--bg-primary, #1a1a1a);
    border: 1px solid var(--border-color, #444);
    border-radius: 8px;
  }
  h3 { margin: 0 0 16px; color: var(--text-primary, #fff); font-size: 16px; }
  label { display: block; margin-bottom: 12px; color: var(--text-secondary, #aaa); font-size: 13px; }
  input {
    display: block; width: 100%; margin-top: 4px; padding: 8px;
    background: var(--bg-secondary, #2a2a2a); color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444); border-radius: 4px; box-sizing: border-box;
  }
  .icon-section { margin-bottom: 12px; }
  .icon-label { color: var(--text-secondary, #aaa); font-size: 13px; }
  .icon-row { display: flex; align-items: center; gap: 8px; margin-top: 4px; }
  .icon-preview {
    width: 40px; height: 40px;
    display: flex; align-items: center; justify-content: center;
    flex-shrink: 0;
  }
  .icon-preview img { width: 32px; height: 32px; border-radius: 6px; }
  .icon-placeholder {
    width: 40px; height: 40px;
    display: flex; align-items: center; justify-content: center;
    background: var(--accent, #4a9eff); color: #fff;
    border-radius: 8px; font-size: 16px; font-weight: 600;
  }
  .icon-btn {
    padding: 6px 12px; background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #ccc); border: 1px solid var(--border-color, #444);
    border-radius: 4px; cursor: pointer; font-size: 12px; white-space: nowrap;
  }
  .icon-btn:hover { border-color: var(--accent, #4a9eff); }
  .icon-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: auto; padding-top: 16px; }
  .cancel {
    padding: 8px 16px; background: transparent; color: var(--text-secondary, #aaa);
    border: 1px solid var(--border-color, #444); border-radius: 4px; cursor: pointer;
  }
  .save {
    padding: 8px 16px; background: var(--accent, #4a9eff); color: #fff;
    border: none; border-radius: 4px; cursor: pointer;
  }
  .save:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
```

**Step 2: Commit**

```bash
git add src/lib/components/EditAppDialog.svelte
git commit -m "feat: create EditAppDialog component with icon picker"
```

---

### Task 5: Wire up dialog routing and sidebar event listener

**Files:**
- Modify: `src/App.svelte:1-36` (add edit-app dialog route)
- Modify: `src/lib/components/Sidebar.svelte:1-61` (listen for edit event, trigger dialog)

**Step 1: Add edit-app route in App.svelte**

In `src/App.svelte`, import `EditAppDialog` and add the route. Add the import:

```typescript
import EditAppDialog from "./lib/components/EditAppDialog.svelte";
```

Add URL param parsing for edit dialog fields (after existing `dialogSpaceId` line):

```typescript
const dialogAppId = params.get("appId") ?? "";
const dialogAppName = params.get("name") ?? "";
const dialogAppUrl = params.get("url") ?? "";
const dialogAppIcon = params.get("icon") ?? "auto";
```

Add the route in the template, after the `add-app` branch:

```svelte
{:else if dialogMode === "edit-app"}
  <EditAppDialog
    spaceId={dialogSpaceId}
    appId={dialogAppId}
    initialName={dialogAppName}
    initialUrl={dialogAppUrl}
    initialIcon={dialogAppIcon}
  />
```

**Step 2: Add event listener in Sidebar.svelte**

In `src/lib/components/Sidebar.svelte`, add a listener for `context-menu-edit-app` in the `onMount` block. Add `showDialog` to the imports from `../api` (already imported). Then add to the `unlisteners.push(...)` call:

```typescript
await listen<{ space_id: string; app_id: string; name: string; url: string; icon: string }>(
  "context-menu-edit-app",
  async (event) => {
    const { space_id, app_id, name, url, icon } = event.payload;
    await showDialog("edit-app", space_id, {
      appId: app_id,
      name,
      url,
      icon,
    });
  }
),
```

**Step 3: Verify frontend compiles**

Run: `npm run check`
Expected: No errors.

**Step 4: Commit**

```bash
git add src/App.svelte src/lib/components/Sidebar.svelte
git commit -m "feat: wire up edit-app dialog routing and sidebar event listener"
```

---

### Task 6: Build and manual test

**Step 1: Full build check**

Run: `npm run tauri dev`
Expected: App launches without errors.

**Step 2: Manual test checklist**

1. Right-click an app in sidebar → context menu shows "Edit" and "Remove"
2. Click "Edit" → edit dialog opens with pre-filled name, URL, icon
3. Change the name → click Save → sidebar updates with new name
4. Click "Choose file..." → native file picker opens, filter shows image types
5. Select an image → icon preview updates in dialog
6. Click Save → sidebar shows new custom icon
7. Open edit again → click "Re-fetch" → icon resets to favicon from URL
8. Press Escape → dialog closes without saving
9. "Remove" context menu item still works as before

**Step 3: Commit**

If any fixes were needed during testing, commit them:
```bash
git add -A
git commit -m "fix: address issues found during edit app testing"
```
