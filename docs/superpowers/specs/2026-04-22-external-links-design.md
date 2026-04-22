# External Links in Default Browser

## Goal

When a user clicks a link inside a service webview that points to a different domain than the service's base URL, open it in the system default browser instead of navigating within the webview. Same-domain links continue to navigate normally inside the webview.

## Approach

Tauri IPC from injected JavaScript (Approach 1 from brainstorming). Two interception points work together:

1. **JS click interceptor** injected into app webviews — catches `<a>` clicks
2. **Updated `on_new_window` handler** — catches `window.open` / `target="_blank"` calls

## Architecture

### New Tauri command: `open_in_browser`

- File: `src-tauri/src/commands/webviews.rs`
- Input: `url: String`
- Implementation: calls `tauri_plugin_shell::open()` on the `AppHandle`
- Returns `Result<(), String>`

### JS click interceptor script

Injected into each app webview after page load. The script:

- Records the page's base hostname at injection time (`window.location.hostname`)
- Adds a `click` event listener on `document` in capture phase
- Walks up from the click target to find the closest `<a>` element
- If the link's `hostname` differs from the base hostname AND the link has an `http`/`https` protocol:
  - Calls `event.preventDefault()`
  - Invokes the `open_in_browser` Tauri command with the link's `href`
- Otherwise allows normal navigation

### Injection mechanism

Use `WebviewBuilder::on_page_load` to inject the script on every page navigation. This ensures the interceptor persists across in-app navigations.

The script uses `__TAURI_INTERNALS__.invoke()` to call the command since app webviews are raw child webviews that don't load the Tauri frontend bundle. The Tauri IPC bridge is injected automatically into all child webviews.

### Updated `on_new_window` handler

Current behavior in `webviews.rs:94-126`:
- OAuth hosts (Google, Apple, Microsoft, GitHub) → create popup window
- Everything else → navigate in same webview, then `Deny`

New behavior:
- OAuth hosts → create popup window (unchanged)
- Same-domain as app → navigate in same webview (unchanged)
- External domain → call `shell.open()` to open in default browser, return `Deny`

To determine "same domain" in `on_new_window`, the app's base hostname must be accessible. This is captured from the `AppConfig.url` field (already available in `open_app`).

## Edge Cases

- **Relative links, `#` anchors, `mailto:`, `tel:`** — The JS checks `a.hostname` which is empty for non-http URLs; these pass through normally.
- **OAuth redirects** — Full-page navigations via server-side redirects. The click interceptor only fires on `<a>` clicks, so OAuth flows are unaffected.
- **`window.open` with external domain** — Handled by the updated `on_new_window` handler.
- **Subdomains** — `docs.google.com` vs `google.com` are treated as different domains. This is the desired behavior for most services.

## Files Changed

- `src-tauri/src/commands/webviews.rs` — add `open_in_browser` command, update `on_new_window`, add `on_page_load` with JS injection
- `src-tauri/src/lib.rs` — register `open_in_browser` in `invoke_handler`
- `src-tauri/capabilities/default.json` — no changes needed (app webviews inherit the shell:allow-open permission via the plugin)

## Testing

- Add an app pointing to a page with external links (e.g., a test HTML page)
- Verify clicking an external-domain `<a href>` opens the system browser
- Verify clicking a same-domain link navigates within the webview
- Verify OAuth flows still work (Google login, etc.)
- Verify `window.open("https://external.com")` opens in the system browser
