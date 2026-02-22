# Edit App Entry + Custom Icon — Design

**Date:** 2026-02-22
**Status:** Approved

## Overview

Add the ability to edit existing app entries in the sidebar and set a custom icon. Users trigger editing via the right-click context menu, which opens an edit dialog with fields for name, URL, and icon (file picker or favicon re-fetch).

## Scope

### In scope
- "Edit" item in app context menu
- Edit dialog with name, URL, icon fields
- File picker for custom icons (png, jpg, svg, ico, webp)
- Re-fetch favicon button
- Icon preview in dialog

### Out of scope
- Inline editing in sidebar
- Icon cropping/resizing
- Drag-and-drop icon upload

## Architecture

### Trigger flow
1. User right-clicks app icon in sidebar → context menu shows "Edit" + "Remove"
2. User clicks "Edit" → Rust emits `context-menu-edit-app` event with app data
3. Sidebar listens for event → calls `showDialog("edit-app", ...)` with app data as URL params
4. Dialog window opens with `EditAppDialog` component pre-filled

### Components modified

**`webviews.rs::show_app_context_menu`** — Add "Edit" menu item (id: `ctx-edit-app`)

**`lib.rs::on_menu_event`** — Handle `ctx-edit-app`:
- Read app data from state using stored context_menu_target
- Emit `context-menu-edit-app` event with space_id, app_id, name, url, icon

**`dialog.rs::show_dialog`** — Accept optional extra params (appId, name, url, icon), append to URL query string. Add "edit-app" title.

**`App.svelte`** — Add `dialog === "edit-app"` routing branch

**`Sidebar.svelte`** — Listen for `context-menu-edit-app`, trigger edit dialog

### Components created

**`EditAppDialog.svelte`** — New dialog component:
- Reads spaceId, appId, name, url, icon from URL params
- Fields: Name (text input), URL (text input), Icon section
- Icon section: preview + "Choose file..." (native dialog) + "Re-fetch favicon" button
- Uses `@tauri-apps/plugin-dialog` for file picker
- On save: calls `editApp()`, emits `dialog-result`, calls `closeDialog()`
- Styled same as AddAppDialog

### Existing code reused (no changes)
- `edit_app` Rust command (apps.rs)
- `editApp()` TypeScript wrapper (api.ts)
- `AppConfig` types (no schema changes)
- `fetch_site_info` for favicon re-fetch

## Dependencies
- `@tauri-apps/plugin-dialog` — for native file open dialog (may need to be added)
- Corresponding Tauri plugin: `tauri-plugin-dialog`
