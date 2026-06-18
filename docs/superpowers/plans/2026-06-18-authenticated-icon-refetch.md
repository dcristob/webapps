# Authenticated Icon Refetch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Edit dialog's "Re-fetch" button capture the favicon from the live, authenticated app webview (auto-opening it if needed) so apps like Gmail/Outlook show their dedicated icon instead of a generic corporate logo.

**Architecture:** A new `refetch_app_icon` command ensures the target webview is open, injects a JS capture script that reads the authenticated DOM's favicon `<link>` URLs, and awaits a `capture_favicon_done` callback (25 s timeout). Rust then downloads the first working URL with the existing `download_favicon` code (the favicon files are public static assets, so unauthenticated download of the *discovered* URL works). A `pending_icon_captures` map in `AppState` bridges the two commands via a `tokio` oneshot channel. The capture script is also injected through the existing `on_page_load(Finished)` hook for the freshly-opened case.

**Tech Stack:** Rust (Tauri v2, reqwest, tokio), Svelte 5 + TypeScript.

## Global Constraints

- Rust backend conventions: `thiserror` for errors, commands in `src-tauri/src/commands/` (one file per domain), all Tauri builder setup in `lib.rs`. Error type from commands is `Result<T, String>`. (From `CLAUDE.md`.)
- HTTP client: `reqwest` with `rustls-tls` (already a dep — do not add `native-tls`). (From `CLAUDE.md`.)
- Svelte 5 runes (`$state`, `$derived`, `$props`); `svelte/store` for cross-component state. Components in `src/lib/components/`. (From `CLAUDE.md`.)
- Conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`). (From `CLAUDE.md`.)
- Platform target: Linux (CachyOS / WebKitGTK). Code must compile and run there; gate Rust changes with `cargo build` + `cargo test` from `src-tauri/`.
- Frontend typecheck command: `npm run check` (svelte-check). NOTE: `CLAUDE.md` mentions `npm run lint` but it is **not** present in `package.json` — use `npm run check` instead.
- The new commands must be registered in the `invoke_handler!` in `src-tauri/src/lib.rs` or the frontend cannot call them.

## Spec Reference

Full design: `docs/superpowers/specs/2026-06-18-authenticated-icon-refetch-design.md`.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `src-tauri/Cargo.toml` | Add explicit `tokio` dep (oneshot, timeout, select!) | Task 1 |
| `src-tauri/src/state.rs` | Add `pending_icon_captures` field to `AppState` | Task 1 |
| `src-tauri/src/lib.rs` | Init the new field; register the 2 new commands | Task 1, 5, 6 |
| `src-tauri/src/commands/favicon.rs` | `download_first_favicon` refactor, `build_favicon_capture_js`, `capture_favicon_done`, `refetch_app_icon` | Task 2, 3, 5, 6 |
| `src-tauri/src/commands/webviews.rs` | `ensure_app_open` refactor; capture injection in `on_page_load` | Task 4, 5 |
| `src/lib/api.ts` | `refetchAppIcon` wrapper | Task 7 |
| `src/lib/components/AppDialog.svelte` | Branch `handleRefetchFavicon` on `mode` | Task 7 |

---

## Task 1: Add `tokio` dependency and `pending_icon_captures` state field

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/state.rs:10-32`
- Modify: `src-tauri/src/lib.rs:68-81`

**Interfaces:**
- Produces: `AppState::pending_icon_captures: Mutex<HashMap<String, tokio::sync::oneshot::Sender<Vec<String>>>>` — keyed by `app_id`, used by Tasks 5 and 6.

- [ ] **Step 1: Add the `tokio` dependency**

Edit `src-tauri/Cargo.toml`. In the `[dependencies]` table, after the `psl = "2"` line, add:

```toml
tokio = { version = "1", features = ["sync", "time", "macros"] }
```

(`sync` for `oneshot`, `time` for `sleep`/`timeout`, `macros` for `tokio::select!`. Tauri already provides the runtime.)

- [ ] **Step 2: Add the `pending_icon_captures` field to `AppState`**

Edit `src-tauri/src/state.rs`. No import changes are needed — `std::sync::Mutex` is already imported (line 2) and `HashMap` is already imported (line 1). In the `AppState` struct, after the `slept_apps` field (line 23) and before the `#[cfg(target_os = "linux")]` block (line 25), add:

```rust
    /// Map of app_id -> oneshot sender awaiting the favicon URL list captured
    /// from that app's live webview by `refetch_app_icon` / `capture_favicon_done`.
    pub pending_icon_captures: Mutex<HashMap<String, tokio::sync::oneshot::Sender<Vec<String>>>>,
```

- [ ] **Step 3: Initialize the field in `lib.rs`**

Edit `src-tauri/src/lib.rs`. In the `AppState { ... }` literal (lines 68-81), after the `slept_apps: ...` line (line 77) and before the `#[cfg(target_os = "linux")]` line (line 78), add:

```rust
            pending_icon_captures: Mutex::new(HashMap::new()),
```

- [ ] **Step 4: Verify it builds and tests pass**

Run:
```bash
cd src-tauri && cargo build
```
Expected: builds with no errors.

Run:
```bash
cd src-tauri && cargo test
```
Expected: all existing tests pass (the `webviews::tests` registrable-domain tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat(state): add pending_icon_captures for icon refetch"
```

---

## Task 2: Extract `download_first_favicon` from `try_download_favicon`

A behavior-preserving refactor so `refetch_app_icon` can reuse the download logic with a JS-provided URL list instead of an HTML string.

**Files:**
- Modify: `src-tauri/src/commands/favicon.rs:44-83`

**Interfaces:**
- Produces: `async fn download_first_favicon(client: &reqwest::Client, urls: &[String], page_url: &str, title: &str) -> String` (returns a local icon path, or `"auto"` if all strategies fail). Consumed by Task 6.
- `try_download_favicon` keeps its existing signature and behavior; it now just extracts URLs from HTML and delegates.

- [ ] **Step 1: Refactor `try_download_favicon`**

Edit `src-tauri/src/commands/favicon.rs`. Replace the entire `try_download_favicon` function (lines 44-83) with this pair of functions:

```rust
/// Try multiple strategies to get a favicon, returning the local path or "auto".
async fn try_download_favicon(
    client: &reqwest::Client,
    html: &str,
    page_url: &str,
    title: &str,
) -> String {
    // Strategy 1: Extract candidate URLs from the page's HTML.
    let favicon_urls = extract_favicon_urls(html, page_url);
    download_first_favicon(client, &favicon_urls, page_url, title).await
}

/// Download the first working favicon from a prioritized URL list, then fall
/// back to the root `/favicon.ico` and Google's favicon service.
///
/// `urls` are strategy-1 candidates (highest priority first). `page_url` is the
/// page the icons belong to, used to build the fallback URLs. Returns a local
/// file path on success or `"auto"` if every strategy fails.
async fn download_first_favicon(
    client: &reqwest::Client,
    urls: &[String],
    page_url: &str,
    title: &str,
) -> String {
    // Strategy 1: try each provided candidate URL in priority order.
    for favicon_url in urls {
        if let Ok(path) = download_favicon(client, favicon_url, title).await {
            return path;
        }
    }

    // Strategy 2: Try /favicon.ico at the root (skip if already in the list).
    if let Ok(parsed) = url::Url::parse(page_url) {
        let root_favicon = format!(
            "{}://{}/favicon.ico",
            parsed.scheme(),
            parsed.host_str().unwrap_or("")
        );
        if !urls.iter().any(|u| u == &root_favicon) {
            if let Ok(path) = download_favicon(client, &root_favicon, title).await {
                return path;
            }
        }
    }

    // Strategy 3: Use Google's favicon service as fallback.
    if let Ok(parsed) = url::Url::parse(page_url) {
        if let Some(domain) = parsed.host_str() {
            let google_url = format!(
                "https://www.google.com/s2/favicons?domain={}&sz=64",
                domain
            );
            if let Ok(path) = download_favicon(client, &google_url, title).await {
                return path;
            }
        }
    }

    "auto".to_string()
}
```

- [ ] **Step 2: Verify it builds and existing tests pass**

Run:
```bash
cd src-tauri && cargo build
```
Expected: builds with no errors.

Run:
```bash
cd src-tauri && cargo test
```
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/favicon.rs
git commit -m "refactor(favicon): extract download_first_favicon helper"
```

---

## Task 3: Add `build_favicon_capture_js` (TDD)

Generates the JS injected into the live webview. It mirrors the priority logic of `extract_favicon_urls` (`apple-touch-icon` > larger `sizes` > generic icon > `og:image`), waits for the page to finish loading plus an 800 ms debounce, then invokes `capture_favicon_done` with the ordered URL list.

**Files:**
- Modify: `src-tauri/src/commands/favicon.rs` (add function + `#[cfg(test)]` module)

**Interfaces:**
- Produces: `pub fn build_favicon_capture_js(app_id: &str) -> String`. Consumed by Tasks 5 (webviews.rs injection) and 6 (refetch_app_icon direct eval).

- [ ] **Step 1: Write the failing tests**

Edit `src-tauri/src/commands/favicon.rs`. At the very end of the file (after `detect_image_format`, line 416), add a test module:

```rust
#[cfg(test)]
mod capture_tests {
    use super::build_favicon_capture_js;

    #[test]
    fn bakes_in_app_id_as_quoted_literal() {
        let js = build_favicon_capture_js("app-123");
        // The app id is embedded as a JS string literal.
        assert!(js.contains(r#"var APP_ID = "app-123";"#));
    }

    #[test]
    fn invokes_capture_favicon_done_command() {
        let js = build_favicon_capture_js("app-1");
        assert!(js.contains("capture_favicon_done"));
        assert!(js.contains("appId"));
        assert!(js.contains("urls"));
    }

    #[test]
    fn has_idempotency_guard() {
        let js = build_favicon_capture_js("app-1");
        // Prevents double-capture if injected twice (e.g. once by refetch's
        // direct eval and once by the on_page_load hook).
        assert!(js.contains("__webapps_icon_captured"));
    }

    #[test]
    fn waits_for_load_with_debounce() {
        let js = build_favicon_capture_js("app-1");
        // Waits for document.readyState === 'complete', then debounces so
        // SPA-set favicons (e.g. Google's client-side <link> injection) settle.
        assert!(js.contains("readyState"));
        assert!(js.contains("800"));
    }

    #[test]
    fn prioritizes_apple_touch_and_larger_sizes() {
        let js = build_favicon_capture_js("app-1");
        // apple-touch-icon gets the top priority.
        assert!(js.contains("apple-touch-icon"));
        assert!(js.contains("priority = 10"));
        // Sorts candidates by priority descending.
        assert!(js.contains("sort"));
    }

    #[test]
    fn resolves_relative_urls_and_includes_og_image() {
        let js = build_favicon_capture_js("app-1");
        assert!(js.contains("new URL("));
        assert!(js.contains("og:image"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
```bash
cd src-tauri && cargo test capture_tests
```
Expected: FAIL — `cannot find function build_favicon_capture_js`.

- [ ] **Step 3: Implement `build_favicon_capture_js`**

Edit `src-tauri/src/commands/favicon.rs`. Add this function immediately before the `#[cfg(test)] mod capture_tests` block you just added:

```rust
/// Build the favicon-capture script injected into an app webview during an
/// icon refetch. Mirrors the priority logic of `extract_favicon_urls`
/// (`apple-touch-icon` > larger `sizes` > generic icon > `og:image`).
///
/// The script waits for the page to finish loading (+800 ms debounce so
/// SPA-injected favicons settle), collects the prioritized favicon URLs from
/// the live DOM, and invokes the `capture_favicon_done` Tauri command with the
/// result. An idempotency guard prevents a second capture if injected twice.
pub fn build_favicon_capture_js(app_id: &str) -> String {
    let app_id_literal = serde_json::to_string(app_id).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"
(function() {{
  if (window.__webapps_icon_captured) return;
  var APP_ID = {app_id_literal};

  function priorityFor(el) {{
    var rel = (el.getAttribute('rel') || '').toLowerCase();
    if (rel.indexOf('apple-touch-icon') !== -1) return 10;
    var sizes = (el.getAttribute('sizes') || '').toLowerCase();
    if (sizes === 'any') return 8; // SVG / scalable, very good
    var m = sizes.match(/(\d+)x\d+/);
    if (m) {{
      var n = parseInt(m[1], 10);
      return n >= 128 ? 7 : n >= 64 ? 5 : n >= 32 ? 3 : 1;
    }}
    return 0;
  }}

  function resolve(href, base) {{
    try {{ return new URL(href, base).href; }}
    catch (e) {{ return null; }}
  }}

  function run() {{
    if (window.__webapps_icon_captured) return;
    window.__webapps_icon_captured = true;

    var base = window.location.href;
    var results = [];
    var links = document.querySelectorAll('link[rel]');
    links.forEach(function(el) {{
      var rel = (el.getAttribute('rel') || '').toLowerCase();
      if (rel.indexOf('icon') === -1) return; // mirrors Rust rel.contains("icon")
      var href = el.getAttribute('href');
      if (!href) return;
      var resolved = resolve(href, base);
      if (resolved) results.push({{ p: priorityFor(el), u: resolved }});
    }});

    // og:image as a last-resort candidate.
    var og = document.querySelector('meta[property="og:image"]');
    if (og) {{
      var content = og.getAttribute('content');
      if (content) {{
        var resolved = resolve(content, base);
        if (resolved) results.push({{ p: -1, u: resolved }});
      }}
    }}

    // Highest priority first.
    results.sort(function(a, b) {{ return b.p - a.p; }});
    var urls = results.map(function(r) {{ return r.u; }});

    if (window.__TAURI_INTERNALS__) {{
      try {{
        window.__TAURI_INTERNALS__.invoke('capture_favicon_done', {{ appId: APP_ID, urls: urls }});
      }} catch (e) {{}}
    }}
  }}

  if (document.readyState === 'complete') {{
    setTimeout(run, 800);
  }} else {{
    window.addEventListener('load', function() {{ setTimeout(run, 800); }});
  }}
}})();
"#
    )
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:
```bash
cd src-tauri && cargo test capture_tests
```
Expected: all 6 `capture_tests` PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/favicon.rs
git commit -m "feat(favicon): add build_favicon_capture_js for authenticated DOM capture"
```

---

## Task 4: Factor `ensure_app_open` out of `open_app`

A behavior-preserving refactor. `open_app` keeps the "wake from sleep" + "already-open → switch" fast path; the webview-creation body becomes `ensure_app_open`, which `refetch_app_icon` (Task 6) calls to create+load a webview without going through the switch fast path.

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs:423-712`

**Interfaces:**
- Produces: `pub fn ensure_app_open(app_handle: &AppHandle, space_id: &str, app_id: &str, state: State<'_, AppState>) -> Result<(), String>`. Consumed by Task 6.
- `open_app` keeps its existing `#[tauri::command] pub fn open_app(...)` signature and external behavior.

- [ ] **Step 1: Split `open_app` into a thin wrapper + `ensure_app_open`**

Edit `src-tauri/src/commands/webviews.rs`. The current `open_app` spans lines 423-712. Its head (423-451) is: signature (423-424), the `(space_clone, app_clone)` extraction (425-433), `label` (435), the sleep-wake block (437-441), and the already-open fast path (443-450). The create body runs from `let data_dir = resolve_data_directory(...)` (452) through `Ok(())` (711).

**Replace lines 423-451** (everything from `#[tauri::command]` down to, but not including, the `let data_dir = ...` line) **with the following** — a new `open_app` wrapper, plus the signature and head of `ensure_app_open` (which re-houses the space/app extraction and `label` that used to be in `open_app`):

```rust
#[tauri::command]
pub fn open_app(app_handle: AppHandle, space_id: String, app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    // Wake from sleep if needed
    {
        let mut slept = state.slept_apps.lock().map_err(|e| e.to_string())?;
        slept.remove(&app_id);
    }

    // Fast path: if the webview already exists, just switch to it.
    {
        let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
        if labels.contains_key(&app_id) {
            drop(labels);
            return switch_to_app(app_handle, space_id, app_id, state);
        }
    }

    // Create the webview (loads the URL, sets it active, emits app-woke).
    ensure_app_open(&app_handle, &space_id, &app_id, state)
}

/// Create and register an app webview. Assumes the webview does NOT already
/// exist (the caller — `open_app` or `refetch_app_icon` — handles the
/// already-open fast path). Performs the full create flow: resolves the data
/// directory, builds the webview with all injected scripts and signal hooks,
/// reparents on Linux, sets it active, and emits `app-woke`.
pub fn ensure_app_open(
    app_handle: &AppHandle,
    space_id: &str,
    app_id: &str,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Extract needed data from the spaces lock, then drop it
    let (space_clone, app_clone) = {
        let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
        let space = spaces.iter().find(|s| s.space.id == space_id)
            .ok_or_else(|| format!("Space '{}' not found", space_id))?;
        let app = space.apps.iter().find(|a| a.id == app_id)
            .ok_or_else(|| format!("App '{}' not found", app_id))?;
        (space.clone(), app.clone())
    };

    let label = format!("app-{}", app_clone.id);

```

After that pasted block, the **existing create body continues verbatim, starting at the original line 452** (`let data_dir = resolve_data_directory(&space_clone, &app_clone)?;`) and ending at the original `Ok(())` / closing brace (lines 711-712). Do **not** add a second `Ok(())`.

Net effect: the space/app extraction and `label` (formerly 425-435) move from `open_app` into `ensure_app_open`; the sleep-wake + fast-path (437-450) stay in `open_app` but now use the `app_id`/`space_id` params directly instead of `app_clone.id`; the create body (452-711) is unchanged. Because `ensure_app_open` takes `app_handle: &AppHandle` and `state: State<'_, AppState>`, every `app_handle.<method>()`, `app_handle.clone()`, and `state.<field>().lock()` in the moved body still resolves (auto-deref / `Deref` on `State`). `s.space.id == space_id` and `a.id == app_id` still compile (`String: PartialEq<&str>`).

- [ ] **Step 2: Verify it builds and existing tests pass**

Run:
```bash
cd src-tauri && cargo build
```
Expected: builds with no errors. (If you get an unused-variable warning for `app_handle` in `open_app`, that's fine — it's passed to `ensure_app_open`.)

Run:
```bash
cd src-tauri && cargo test
```
Expected: all `webviews::tests` pass.

- [ ] **Step 3: Smoke-test that opening apps still works**

Run the app:
```bash
npm run tauri dev
```
Expected: click an app in the sidebar → its webview opens and shows content exactly as before. Switching between apps still works. (This guards against the refactor breaking the create flow.) Stop the dev server when done.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs
git commit -m "refactor(webviews): extract ensure_app_open from open_app"
```

---

## Task 5: Add `capture_favicon_done` command and inject the capture script on page load

Adds the receiver half of the bridge (`capture_favicon_done` resolves the pending oneshot) and wires the capture-script injection into the existing `on_page_load(Finished)` hook so the freshly-opened case (Task 6 opens the webview) gets captured automatically.

**Files:**
- Modify: `src-tauri/src/commands/favicon.rs` (add `capture_favicon_done`)
- Modify: `src-tauri/src/commands/webviews.rs:479-608` (add `app_id_for_capture`; inject in `on_page_load`)
- Modify: `src-tauri/src/lib.rs:290-325` (register `capture_favicon_done`)

**Interfaces:**
- Produces: `#[tauri::command] pub fn capture_favicon_done(app_id: String, urls: Vec<String>, state: State<'_, AppState>) -> Result<(), String>`. Invoked from JS and registered so `invoke` works.
- Consumes: `AppState::pending_icon_captures` (Task 1), `build_favicon_capture_js` (Task 3).

- [ ] **Step 1: Add the `capture_favicon_done` command**

Edit `src-tauri/src/commands/favicon.rs`. Add the `tauri::State` import at the top of the file. Change line 3-5:

```rust
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};

use crate::config::storage;
```

to:

```rust
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use tauri::State;

use crate::config::storage;
use crate::state::AppState;
```

Then add this command immediately before `pub fn build_favicon_capture_js` (i.e., right after the `detect_image_format` function, before the test module is fine too — place it just above `build_favicon_capture_js`):

```rust
/// Receiver half of the icon-capture bridge. Called from JS (via
/// `__TAURI_INTERNALS__.invoke`) once `build_favicon_capture_js` has collected
/// the favicon URLs from the live webview DOM. Resolves the oneshot that
/// `refetch_app_icon` is awaiting. No-op if no capture is pending for this app
/// (e.g. a stale injection arriving after `refetch_app_icon` timed out).
#[tauri::command]
pub fn capture_favicon_done(app_id: String, urls: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut pending = state.pending_icon_captures.lock().map_err(|e| e.to_string())?;
    if let Some(sender) = pending.remove(&app_id) {
        // Receiver may have been dropped on timeout; ignore send errors.
        let _ = sender.send(urls);
    }
    Ok(())
}
```

- [ ] **Step 2: Inject the capture script in `on_page_load(Finished)`**

Edit `src-tauri/src/commands/webviews.rs`.

First, add the import. Near the top of the file, after `use crate::state::AppState;` (line 13), add:

```rust
use crate::commands::favicon::build_favicon_capture_js;
```

Next, add clones for the capture closure. Find the line (around line 479) `let app_id_for_title = app_clone.id.clone();` and immediately after it add:

```rust
    let app_id_for_capture = app_clone.id.clone();
    let app_handle_for_capture = app_handle.clone();
```

(We capture an `AppHandle` clone — matching how the other closures in `open_app` capture `app_handle_for_title` / `app_handle_for_nav` — rather than relying on a webview method. This keeps `app_handle` itself available for the later Linux hooks.)

Then find the `on_page_load` block (around lines 599-608):

```rust
    let webview_builder = webview_builder
        .on_page_load(move |webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let _ = webview.eval(MEDIA_GUARD_JS);
                let _ = webview.eval(&window_open_override_js);
            }
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.eval(&link_interceptor_js);
            }
        });
```

Replace the `Finished` branch so it also injects the capture script when a refetch is pending for this app:

```rust
    let webview_builder = webview_builder
        .on_page_load(move |webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let _ = webview.eval(MEDIA_GUARD_JS);
                let _ = webview.eval(&window_open_override_js);
            }
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.eval(&link_interceptor_js);
                // If an icon refetch is pending for this app, inject the
                // capture script (it waits for load + debounce, then reports
                // the authenticated DOM's favicon URLs back to Rust).
                let state = app_handle_for_capture.state::<AppState>();
                if let Ok(guard) = state.pending_icon_captures.lock() {
                    if guard.contains_key(&app_id_for_capture) {
                        let _ = webview.eval(&build_favicon_capture_js(&app_id_for_capture));
                    }
                }
            }
        });
```

(`app_handle_for_capture` and `app_id_for_capture` are captured by the `move` closure. `app_handle.state::<T>()` is the same `Manager` method already used at `webviews.rs:274` and `lib.rs:186`. The `Manager` trait is already imported at the top of `webviews.rs`.)

- [ ] **Step 3: Register `capture_favicon_done` in the invoke handler**

Edit `src-tauri/src/lib.rs`. In the `tauri::generate_handler![...]` list (lines 290-325), after the line `commands::favicon::fetch_site_info,` (line 318), add:

```rust
            commands::favicon::capture_favicon_done,
```

- [ ] **Step 4: Verify it builds and tests pass**

Run:
```bash
cd src-tauri && cargo build
```
Expected: builds with no errors.

Run:
```bash
cd src-tauri && cargo test
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/favicon.rs src-tauri/src/commands/webviews.rs src-tauri/src/lib.rs
git commit -m "feat(favicon): add capture_favicon_done bridge and on_page_load injection"
```

---

## Task 6: Add the `refetch_app_icon` command

The orchestrator. Ensures the target webview is open (creating it via `ensure_app_open` if needed), injects the capture script (directly if already open, or via the `on_page_load` hook from Task 5 if just created), awaits the `capture_favicon_done` callback with a 25 s timeout, downloads the first working URL, restores the previously-active app, and returns the new icon path.

**Files:**
- Modify: `src-tauri/src/commands/favicon.rs` (add `refetch_app_icon`)
- Modify: `src-tauri/src/lib.rs:290-325` (register `refetch_app_icon`)

**Interfaces:**
- Consumes: `AppState::pending_icon_captures` (Task 1), `download_first_favicon` (Task 2), `build_favicon_capture_js` (Task 3), `commands::webviews::ensure_app_open` (Task 4), `commands::webviews::switch_to_app` (existing).
- Produces: `#[tauri::command] async fn refetch_app_icon(app_handle, space_id, app_id, state) -> Result<String, String>` returning the new local icon path. Called from the frontend (Task 7).

- [ ] **Step 1: Add the `refetch_app_icon` command**

Edit `src-tauri/src/commands/favicon.rs`. Add these imports near the top (after the `use crate::state::AppState;` added in Task 5):

```rust
use std::time::Duration;
use tauri::{AppHandle, Manager};
```

(`Manager` gives `.state::<T>()` and `.get_webview(...)` on `AppHandle`; `State` was imported in Task 5.)

Then add this command immediately before `pub fn build_favicon_capture_js` (place it right after `capture_favicon_done`):

```rust
/// Capture the favicon from an app's live (authenticated) webview.
///
/// If the webview is already open, the capture script is eval'd directly
/// against the loaded page. Otherwise the webview is created via
/// `ensure_app_open` (its persisted cookies load the authenticated page),
/// and the `on_page_load(Finished)` hook injects the capture script on first
/// load. The script reports the DOM's favicon URLs back via
/// `capture_favicon_done`; this command awaits that with a 25 s timeout, then
/// downloads the first working URL. The previously-active app is restored
/// afterwards so the user's view isn't hijacked by the auto-open.
///
/// Returns the new local icon path. Does NOT persist it to the app config —
/// the frontend's Save action does that via `edit_app`, consistent with other
/// edits.
#[tauri::command]
pub async fn refetch_app_icon(
    app_handle: AppHandle,
    space_id: String,
    app_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Resolve the target app + remember the currently-active app to restore later.
    let (app_url, app_name, prev_active) = {
        let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
        let app = spaces
            .iter()
            .flat_map(|s| s.apps.iter())
            .find(|a| a.id == app_id)
            .ok_or_else(|| format!("App '{}' not found", app_id))?;
        let url = app.url.clone();
        let name = app.name.clone();
        let active = state.active_app_id.lock().map_err(|e| e.to_string())?.clone();
        (url, name, active)
    };

    // Refuse a double-refetch for the same app (would orphan the first awaiter).
    {
        let pending = state.pending_icon_captures.lock().map_err(|e| e.to_string())?;
        if pending.contains_key(&app_id) {
            return Err("An icon refetch is already in progress for this app".to_string());
        }
    }

    // Register the oneshot BEFORE opening / eval'ing, so the on_page_load hook
    // and the direct eval both see a pending entry.
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<String>>();
    {
        let mut pending = state.pending_icon_captures.lock().map_err(|e| e.to_string())?;
        pending.insert(app_id.clone(), tx);
    }

    // Decide injection path based on whether the webview already exists.
    let already_open = {
        let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
        labels.contains_key(&app_id)
    };
    if already_open {
        // Page is loaded; eval the capture script directly (no view switch).
        let js = build_favicon_capture_js(&app_id);
        if let Some(webview) = app_handle.get_webview(&format!("app-{}", app_id)) {
            let _ = webview.eval(&js);
        } else {
            // Raced with a close — clean up and bail.
            let mut pending = state.pending_icon_captures.lock().map_err(|e| e.to_string())?;
            pending.remove(&app_id);
            return Err("App webview is not available".to_string());
        }
    } else {
        // Create + load the webview. Its on_page_load(Finished) hook injects
        // the capture script (pending entry is already set above).
        crate::commands::webviews::ensure_app_open(&app_handle, &space_id, &app_id, state)?;
    }

    // Await the captured URLs (or time out).
    let urls = tokio::select! {
        result = rx => result.map_err(|_| "Capture cancelled before completion".to_string())?,
        _ = tokio::time::sleep(Duration::from_secs(25)) => {
            {
                let mut pending = state.pending_icon_captures.lock().map_err(|e| e.to_string())?;
                pending.remove(&app_id);
            }
            // Best-effort: restore the previous view before reporting failure.
            let _ = restore_previous(&app_handle, &space_id, &app_id, &prev_active);
            return Err("Timed out waiting for the app page to load".to_string());
        }
    };

    // Clean up the pending entry (capture_favicon_done already removed it, but
    // guard against a path where it didn't).
    {
        let mut pending = state.pending_icon_captures.lock().map_err(|e| e.to_string())?;
        pending.remove(&app_id);
    }

    // Download the first working URL (same priority/fallbacks as fetch_site_info).
    let client = http_client()?;
    let icon_path = download_first_favicon(&client, &urls, &app_url, &app_name).await;
    if icon_path == "auto" {
        // No usable favicon found in the authenticated DOM either.
        let _ = restore_previous(&app_handle, &space_id, &app_id, &prev_active);
        return Err("No favicon found on the authenticated page".to_string());
    }

    let _ = restore_previous(&app_handle, &space_id, &app_id, &prev_active);
    Ok(icon_path)
}

/// Restore the previously-active app's view after an auto-open refetch.
/// Derives `State` from `app_handle` (via `Manager::state`) so it needs no
/// borrowed `State` argument.
fn restore_previous(
    app_handle: &AppHandle,
    space_id: &str,
    refetched_app_id: &str,
    prev_active: &Option<String>,
) -> Result<(), String> {
    if let Some(prev) = prev_active.as_deref() {
        if prev != refetched_app_id {
            let state = app_handle.state::<AppState>();
            let prev_still_open = {
                let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
                labels.contains_key(prev)
            };
            if prev_still_open {
                // switch_to_app ignores its space_id arg, so passing the
                // refetched app's space is harmless.
                crate::commands::webviews::switch_to_app(
                    app_handle.clone(),
                    space_id.to_string(),
                    prev.to_string(),
                    state,
                )?;
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Register `refetch_app_icon` in the invoke handler**

Edit `src-tauri/src/lib.rs`. In the `tauri::generate_handler![...]` list, immediately after the `commands::favicon::capture_favicon_done,` line added in Task 5, add:

```rust
            commands::favicon::refetch_app_icon,
```

- [ ] **Step 3: Verify it builds and tests pass**

Run:
```bash
cd src-tauri && cargo build
```
Expected: builds with no errors.

Run:
```bash
cd src-tauri && cargo test
```
Expected: all tests pass.

Run:
```bash
cd src-tauri && cargo clippy
```
Expected: no new warnings introduced by this task (pre-existing warnings, if any, are not blocking — just confirm none of *our* code triggers one).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/favicon.rs src-tauri/src/lib.rs
git commit -m "feat(favicon): add refetch_app_icon to capture authenticated favicon"
```

---

## Task 7: Frontend — `refetchAppIcon` API wrapper + Edit-dialog mode branch

**Files:**
- Modify: `src/lib/api.ts:105-108`
- Modify: `src/lib/components/AppDialog.svelte:1-6,71-81`

**Interfaces:**
- Consumes: the `refetch_app_icon` command (Task 6).
- Produces: `refetchAppIcon(spaceId, appId)` returning the new icon path; the Edit dialog's Re-fetch button uses it in edit mode.

- [ ] **Step 1: Add the `refetchAppIcon` wrapper**

Edit `src/lib/api.ts`. Find the Favicon section (lines 105-108):

```ts
// Favicon
export async function fetchSiteInfo(url: string): Promise<[string, string]> {
  return invoke("fetch_site_info", { url });
}
```

Immediately after it, add:

```ts
export async function refetchAppIcon(spaceId: string, appId: string): Promise<string> {
  return invoke("refetch_app_icon", { spaceId, appId });
}
```

- [ ] **Step 2: Branch `handleRefetchFavicon` on mode in the Edit dialog**

Edit `src/lib/components/AppDialog.svelte`. Update the import on line 5 from:

```ts
  import { addApp, editApp, fetchSiteInfo, closeDialog } from "../api";
```

to:

```ts
  import { addApp, editApp, fetchSiteInfo, refetchAppIcon, closeDialog } from "../api";
```

Then replace the `handleRefetchFavicon` function (lines 71-81):

```ts
  async function handleRefetchFavicon() {
    if (!url.trim()) return;
    loading = true;
    try {
      const [, fetchedIcon] = await fetchSiteInfo(url.trim());
      icon = fetchedIcon;
    } catch {
      // Keep current icon on error
    }
    loading = false;
  }
```

with:

```ts
  async function handleRefetchFavicon() {
    if (!url.trim()) return;
    loading = true;
    try {
      if (mode === "edit" && appId) {
        // Capture the favicon from the live (authenticated) webview, auto-
        // opening the app if needed. Returns the new local icon path.
        const fetchedIcon = await refetchAppIcon(spaceId, appId);
        icon = fetchedIcon;
      } else {
        // Add mode: no app/webview exists yet, so use the generic fetch.
        const [, fetchedIcon] = await fetchSiteInfo(url.trim());
        icon = fetchedIcon;
      }
    } catch {
      // Keep current icon on error
    }
    loading = false;
  }
```

- [ ] **Step 3: Verify the frontend type-checks and builds**

Run:
```bash
npm run check
```
Expected: 0 errors, 0 warnings.

Run:
```bash
npm run build
```
Expected: vite build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/lib/api.ts src/lib/components/AppDialog.svelte
git commit -m "feat(ui): use authenticated refetch for Edit dialog Re-fetch button"
```

---

## Task 8: Manual end-to-end verification

No code changes. Confirms the spec's manual test list. Run the production-style build (frontend embedded) per `CLAUDE.md`.

**Files:** none.

- [ ] **Step 1: Build the production binary**

```bash
npm run tauri -- build --no-bundle
```
Expected: succeeds, prints an embedded asset path under `src-tauri/target/release/`.

Verify the frontend was embedded:
```bash
strings src-tauri/target/release/webapps | grep -o 'assets/index-[A-Za-z0-9_]*\.js'
```
Expected: prints a non-empty asset filename.

- [ ] **Step 2: Baseline — first add still gives the generic icon (unchanged behavior)**

Launch `~/.local/bin/webapps` (or the dev build). Close any running instance first.

Add a new app with URL `https://mail.google.com` (or `https://outlook.office.com`).
Expected: the app is added with a generic Google/Microsoft corporate icon (this confirms we did not regress the initial-add path).

- [ ] **Step 3: Authenticate, then Edit → Re-fetch yields the dedicated icon**

Open the added app and complete sign-in. Then right-click the app → **Edit** → click **Re-fetch**.
Expected: after a few seconds the icon preview updates to the dedicated icon (Gmail envelope / Outlook icon). Click **Save**; the sidebar shows the dedicated icon.

- [ ] **Step 4: Was-closed case (auto-open)**

Close the app's webview (or sleep it), so it is no longer loaded. With a *different* app currently active, open the target app's Edit dialog → **Re-fetch**.
Expected: the target app's webview auto-opens (briefly becomes active), captures the icon, then the view returns to the previously-active app. The Edit dialog preview shows the dedicated icon.

- [ ] **Step 5: Timeout / error handling**

Disconnect network, then Edit → **Re-fetch** on an app.
Expected: after ~25 s (or sooner if the webview fails to load), the Re-fetch resolves, the icon is unchanged, and no crash occurs. (If a toast UI exists, it surfaces the error; otherwise the silent "keep current icon" fallback applies.)

- [ ] **Step 6: Add-mode Re-fetch still uses the generic fetch**

Add a *new* app (add dialog), click **Fetch**, then click **Re-fetch**.
Expected: both use the generic `fetchSiteInfo` (no auto-open / no error), since no app/webview exists yet.

- [ ] **Step 7: No final commit needed**

This task is verification only. If all steps pass, the feature is complete. Update the local install per `CLAUDE.md` if desired:
```bash
cp -f src-tauri/target/release/webapps ~/.local/bin/webapps
```

---

## Self-Review (completed during planning)

**Spec coverage:** Every spec section maps to a task — `pending_icon_captures` (Task 1), `download_first_favicon` refactor (Task 2), `build_favicon_capture_js` (Task 3), `ensure_app_open` refactor (Task 4), `capture_favicon_done` + `on_page_load` injection + registration (Task 5), `refetch_app_icon` + registration (Task 6), frontend wrapper + Edit-dialog branch (Task 7), manual edge-case list incl. timeout/restore/add-mode (Task 8). ✓

**Type/name consistency:** `download_first_favicon(client, urls, page_url, title)` defined (Task 2) and called identically (Task 6). `build_favicon_capture_js(app_id)` defined (Task 3), used in `on_page_load` (Task 5) and `refetch_app_icon` (Task 6). `ensure_app_open(app_handle, space_id, app_id, state)` defined (Task 4), called from `refetch_app_icon` (Task 6). `capture_favicon_done(app_id, urls, state)` defined (Task 5), invoked from JS as `capture_favicon_done` with `{ appId, urls }` (Task 3 JS) — Tauri maps `appId`→`app_id`. `refetchAppIcon(spaceId, appId)` wrapper (Task 7) matches command args `space_id, app_id`. ✓

**Placeholder scan:** No TBD/TODO; every code step contains the full code; refactors reference exact line ranges and describe the mechanical move verbatim (the body is not re-typed because it is unchanged). ✓

**Note on Tauri APIs used:** All APIs (`app_handle.get_webview(label)`, `app_handle.state::<T>()`, `webview.eval`, `Manager` trait) are already used elsewhere in `src-tauri/src/commands/webviews.rs` and `src-tauri/src/lib.rs`, so they match the project's Tauri 2 version exactly. The webview label format `app-{id}` matches `webviews.rs:435`.
