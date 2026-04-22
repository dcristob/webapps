# External Links in Default Browser — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open external-domain links from service webviews in the system default browser, while allowing same-domain navigation within the webview.

**Architecture:** Two interception points: (1) a JS click interceptor injected via `on_page_load` catches `<a>` clicks and sends external URLs to a new Tauri command `open_in_browser`; (2) the existing `on_new_window` handler is updated to open external-domain `window.open` calls in the browser. The app's base hostname is extracted from the `AppConfig.url` field.

**Tech Stack:** Rust (Tauri v2, tauri-plugin-shell), JavaScript (injected into webviews)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/commands/webviews.rs` | Modify | Add `open_in_browser` command, update `on_new_window`, add `on_page_load` with JS injection |
| `src-tauri/src/lib.rs` | Modify | Register `open_in_browser` in `invoke_handler` |
| `src-tauri/capabilities/default.json` | Modify | Add `"app-*"` glob pattern to webviews array with `shell:allow-open` permission |

---

### Task 1: Add capability for app webviews

**Files:**
- Modify: `src-tauri/capabilities/default.json`

App webviews have labels like `app-<uuid>`. They need permission to invoke `open_in_browser` which calls `shell.open`. Add a glob pattern `"app-*"` to the webviews array and ensure the `shell:allow-open` permission covers it.

- [ ] **Step 1: Update capabilities**

Edit `src-tauri/capabilities/default.json` to add `"app-*"` glob:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for sidebar, topbar, dialog, and app webviews",
  "webviews": ["sidebar", "topbar", "dialog", "app-*"],
  "permissions": [
    "core:default",
    "core:event:default",
    "shell:allow-open",
    "dialog:allow-open"
  ]
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

- [ ] **Step 3: Commit**

```bash
git add src-tauri/capabilities/default.json
git commit -m "feat: add app webview capability for shell:allow-open"
```

---

### Task 2: Add `open_in_browser` Tauri command

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`
- Modify: `src-tauri/src/lib.rs`

Add a new Tauri command that takes a URL string and opens it in the default browser using `tauri_plugin_shell`'s `open` function.

- [ ] **Step 1: Add the command to `webviews.rs`**

Add this function at the end of `src-tauri/src/commands/webviews.rs` (before `parse_badge_count`):

```rust
#[tauri::command]
pub fn open_in_browser(app_handle: AppHandle, url: String) -> Result<(), String> {
    let shell = app_handle.shell();
    shell.open(url.clone(), None).map_err(|e| e.to_string())?;
    Ok(())
}
```

Note: `app_handle.shell()` returns the `Shell` extension. If that method is not available, use the alternative:

```rust
#[tauri::command]
pub fn open_in_browser(app_handle: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app_handle.shell().open(url, None).map_err(|e| e.to_string())
}
```

Check which API is available by looking at the trait: `grep "pub trait ShellExt\|fn shell(" ~/.cargo/registry/src/*/tauri-plugin-shell-*/src/*.rs`. The plugin exposes `.shell()` via the `ShellExt` trait on `AppHandle` — make sure to import it.

- [ ] **Step 2: Register in `lib.rs` invoke_handler**

Add `commands::webviews::open_in_browser` to the `invoke_handler` macro in `src-tauri/src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::webviews::open_in_browser,
])
```

- [ ] **Step 3: Verify build compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs src-tauri/src/lib.rs
git commit -m "feat: add open_in_browser Tauri command"
```

---

### Task 3: Inject JS click interceptor via `on_page_load`

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`

Add an `on_page_load` handler to the `WebviewBuilder` in `open_app` that injects a JavaScript click interceptor on every page load. The script uses `window.__TAURI_INTERNALS__.invoke()` to call `open_in_browser` for external links.

- [ ] **Step 1: Define the JS click interceptor script**

Add a constant at the top of `webviews.rs` (after the `USER_AGENT` constant, around line 17) containing the interceptor script:

```rust
const LINK_INTERCEPTOR_JS: &str = r#"
(function() {
  var baseHostname = window.location.hostname;

  document.addEventListener('click', function(e) {
    var el = e.target;
    while (el && el.tagName !== 'A') {
      el = el.parentElement;
    }
    if (!el) return;

    var href = el.href;
    if (!href) return;

    var protocol = el.protocol;
    if (protocol !== 'http:' && protocol !== 'https:') return;

    if (el.hostname !== baseHostname) {
      e.preventDefault();
      e.stopPropagation();
      if (window.__TAURI_INTERNALS__) {
        window.__TAURI_INTERNALS__.invoke('open_in_browser', { url: href });
      }
    }
  }, true);
})();
"#;
```

- [ ] **Step 2: Add `on_page_load` to the webview builder**

In the `open_app` function, add `.on_page_load()` to the webview builder chain (after `.on_document_title_changed()` and before `window.add_child()`). The handler injects the JS only on `Finished` page loads:

```rust
let app_hostname = app_clone.url.clone();
let webview_builder = webview_builder
    .on_page_load(move |webview, payload| {
        if payload.event() == tauri::webview::PageLoadEvent::Finished {
            let _ = webview.eval(LINK_INTERCEPTOR_JS);
        }
    });
```

This goes after the `on_document_title_changed` closure (line 137 in the current code) and before the `window.add_child()` call (line 139).

Note: we capture `app_hostname` here for potential future use but the JS script itself reads `window.location.hostname` at injection time which is always correct.

- [ ] **Step 3: Verify build compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs
git commit -m "feat: inject JS click interceptor for external links"
```

---

### Task 4: Update `on_new_window` handler for external domains

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`

Currently `on_new_window` (line 94) creates popups for OAuth hosts and navigates everything else in the same webview. Update it to open external-domain `window.open` calls in the default browser.

- [ ] **Step 1: Determine the app's base hostname in the closure**

The `on_new_window` closure already has access to `app_handle_for_nav` and `label_for_nav`. We need to also capture the app's base URL hostname. Add a new captured variable before the `on_new_window` closure:

```rust
let base_url_host = url::Url::parse(&app_clone.url)
    .ok()
    .and_then(|u| u.host_str().map(|h| h.to_string()));
```

Place this right after `let label_for_nav = label.clone();` (line 88) and before the webview builder.

- [ ] **Step 2: Update the `on_new_window` handler**

Replace the current `on_new_window` handler (lines 94-126) with:

```rust
.on_new_window(move |url, features| {
    let host = url.host_str().unwrap_or("");

    if host.ends_with("accounts.google.com")
        || host.ends_with("appleid.apple.com")
        || host.ends_with("login.microsoftonline.com")
        || host.ends_with("github.com")
    {
        let popup_id = POPUP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let popup_label = format!("popup-{}", popup_id);
        if let Ok(window) = WebviewWindowBuilder::new(
            &app_handle_for_nav,
            &popup_label,
            WebviewUrl::External("about:blank".parse().unwrap()),
        )
        .window_features(features)
        .user_agent(USER_AGENT)
        .inner_size(500.0, 700.0)
        .title(url.as_str())
        .build()
        {
            return NewWindowResponse::Create { window };
        }
        return NewWindowResponse::Allow;
    }

    let is_external = base_url_host
        .as_ref()
        .map(|base| host != base.as_str())
        .unwrap_or(true);

    if is_external {
        use tauri_plugin_shell::ShellExt;
        let _ = app_handle_for_nav.shell().open(url.to_string(), None);
        return NewWindowResponse::Deny;
    }

    if let Some(webview) = app_handle_for_nav.get_webview(&label_for_nav) {
        let url_str = url.as_str().replace('\\', "\\\\").replace('\'', "\\'");
        let _ = webview.eval(&format!("window.location.href = '{}'", url_str));
    }
    NewWindowResponse::Deny
})
```

- [ ] **Step 3: Verify build compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs
git commit -m "feat: open external window.open URLs in default browser"
```

---

### Task 5: Manual testing

No automated tests for this feature (it involves external browser behavior). Manual verification:

- [ ] **Step 1: Build and run the app**

Run: `npm run tauri dev`

- [ ] **Step 2: Test external `<a>` link click**

Add an app pointing to a page with external links (e.g., Wikipedia). Click an external link. Expected: opens in system browser, webview stays on the same page.

- [ ] **Step 3: Test same-domain navigation**

Click a same-domain link on the same page. Expected: navigates within the webview normally.

- [ ] **Step 4: Test OAuth flow**

Try logging into a Google service. Expected: popup window opens for OAuth, login succeeds.

- [ ] **Step 5: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix: address external link edge cases"
```
