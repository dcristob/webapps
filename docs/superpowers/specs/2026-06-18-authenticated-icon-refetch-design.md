# Authenticated Icon Refetch

**Date:** 2026-06-18
**Status:** Approved (design)
**Scope:** Phase 1 — icon retrieval enhancement

## Problem

When a new web app is added, `fetch_site_info` (`src-tauri/src/commands/favicon.rs:26`)
retrieves the favicon via an **unauthenticated** `reqwest` GET with a browser-like
User-Agent but no session cookies. For Google and Microsoft SaaS apps, that request
redirects to a login page (`accounts.google.com`, `login.microsoftonline.com`) whose
`<link rel="icon">` is the corporate logo — not the dedicated Gmail / Outlook / Drive
icon.

The Edit dialog's existing "Re-fetch" button (`src/lib/components/AppDialog.svelte:71`)
calls the same unauthenticated path, so it always returns the same generic icon. The
dedicated icon URL only appears in the **authenticated** app page's DOM.

The initial generic-icon behavior on first add is acceptable (the user confirmed this).
The goal is: once the user has authenticated, "Re-fetch" in the Edit dialog should
capture the real, dedicated icon.

## Decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Scope | Authenticated refetch only. Initial-add behavior unchanged. |
| Fallback when webview isn't open | Auto-open the app webview, then capture. |
| Trigger location | Edit dialog only (upgrade existing "Re-fetch"). |
| Byte-acquisition mechanism | JS extracts URLs from the authenticated DOM; Rust downloads them with existing code (the favicon files are public static assets, so unauthenticated download of the *discovered* URL works for Google/Microsoft). |

## Solution Overview

Upgrade the Edit dialog's "Re-fetch" to capture favicon URLs from the **live
authenticated webview**, then download them with existing Rust code.

Key insight: authentication matters for *discovering* the right favicon URL (the
dedicated icon's `<link>` only appears in the authenticated page's DOM), but the
favicon *file itself* is normally a public static asset. So JS in the webview extracts
the prioritized URL list; Rust downloads it unauthenticated (reusing the existing
`download_favicon` / `save_icon` / `detect_image_format` code).

### End-to-end flow

1. User opens the app and signs in. Session cookies persist in
   `~/.config/webapps/webview-data/space-<id>/...`.
2. User opens the **Edit** dialog for that app and clicks **Re-fetch**.
3. New command `refetch_app_icon(space_id, app_id)`:
   1. Remembers the currently-active app (to restore the view afterwards).
   2. Ensures the target webview is open — opens it if closed or slept. Because its
      cookies persist, it loads the **authenticated** app page directly (not the login
      page).
   3. Injects a capture script that, once the page is loaded, collects prioritized
      favicon `<link>` URLs from the DOM and calls back via a Tauri command.
   4. Awaits the callback with a timeout (25 s).
   5. Downloads the first working URL via existing `download_favicon` (+ root
      `/favicon.ico` and Google-service fallbacks), saves via `save_icon`.
   6. Restores the previously-active app view, returns the new icon path.
4. The Edit dialog's icon preview updates. The new icon is persisted when the user
   clicks **Save** (consistent with how name/url/icon edits are only saved on Save).

## Backend Changes

### `src-tauri/src/state.rs`

Add one field to `AppState`:

```rust
pub pending_icon_captures: std::sync::Mutex<
    std::collections::HashMap<String, tokio::sync::oneshot::Sender<Vec<String>>>,
>,
```

Keyed by `app_id`; carries the ordered favicon URL list back from the webview to the
awaiting `refetch_app_icon` call.

### `src-tauri/src/commands/favicon.rs`

Add:

- `build_favicon_capture_js(app_id: &str) -> String` — generated JS that mirrors the
  priority logic of `extract_favicon_urls` (`apple-touch-icon` > larger `sizes` > generic
  icon > `og:image`, sorted descending). Behaviour:
  - Idempotency guard (`window.__webapps_icon_captured`) prevents double-capture if
    injected twice.
  - Waits until `document.readyState === 'complete'`, then a `setTimeout(…, 800)` debounce
    so SPA-set favicons (e.g. Google's client-side `<link>` injection) settle.
  - Builds the prioritized URL list (resolving relative hrefs against
    `window.location.href`), then invokes `capture_favicon_done` with
    `{ appId, urls }` via `window.__TAURI_INTERNALS__.invoke`.
- `#[tauri::command] async fn refetch_app_icon(app_handle, space_id, app_id, state) -> Result<String, String>`
  — orchestrates the flow above. Returns the new icon path on success.
- `#[tauri::command] fn capture_favicon_done(app_id, urls, state) -> Result<(), String>`
  — looks up the pending `oneshot::Sender` for `app_id`, sends the URL list, removes the
  entry. No-op (returns Ok) if no pending capture exists (e.g. stale injection after a
  timeout cleanup).

Refactor:

- Extract `download_first_favicon(client, urls, page_url, title) -> String` from
  `try_download_favicon`. It iterates the provided URLs (strategy 1), then appends the
  root `/favicon.ico` (strategy 2) and Google favicon service (strategy 3) fallbacks —
  the same logic as today. Both `fetch_site_info` (which feeds it `extract_favicon_urls`
  output) and `refetch_app_icon` (which feeds it the JS-extracted URLs) reuse it. Keeps
  behaviour identical for `fetch_site_info`.

#### `refetch_app_icon` orchestration details

1. Lock `spaces`, clone the target `(SpaceConfig, AppConfig)`. Read
   `state.active_app_id` → `prev_active`.
2. If a capture is already pending for `app_id` (`pending_icon_captures` has the key),
   return early with a "refetch already in progress" error (avoids orphaning the first
   awaiter).
3. Create a `tokio::sync::oneshot::channel::<Vec<String>>()`, insert the sender into
   `pending_icon_captures[app_id]`.
4. Decide injection path:
   - If `app_id` is in `webview_labels` (webview already exists) → the page is already
     loaded; eval `build_favicon_capture_js` directly via `get_webview(label).eval(…)`.
     No view switch.
   - Else → the `on_page_load(Finished)` hook in `webviews.rs` will eval the capture
     script on first load (see below). Call the open path (reuse `open_app`'s logic —
     factored into an inner helper `ensure_app_open` that returns whether it was newly
     created) so the webview is created and starts loading the authenticated page.
5. `tokio::select!` on the receiver vs. a 25 s `tokio::time::sleep`. On timeout, remove
   the pending entry, return an error ("Timed out waiting for the page to load").
6. On success (`Vec<String>` of URLs): call
   `download_first_favicon(&client, urls, &app.url, &app.name)` → local icon path. Build
   the reqwest client via the existing `http_client()`.
7. Restore the previous view: if `prev_active` is `Some(other)` and `other != app_id`
   and `other` is still in `webview_labels`, call `switch_to_app(other)`. (If the target
   was already active, or there was no previous active app, leave the view as-is.)
8. Remove `pending_icon_captures[app_id]`. Return the icon path.

The command does **not** persist the icon to the app config — the frontend's Save
action does that via the existing `edit_app` command, consistent with how other edits
behave. (The auto-open side-effect on the view is mitigated by step 7's restore.)

### `src-tauri/src/commands/webviews.rs`

In the existing `on_page_load(Finished)` handler (`webviews.rs:605`), add: if `app_id`
is present in `state.pending_icon_captures`, eval `build_favicon_capture_js(app_id)`.
This is the injection point for the freshly-opened case. (For already-open webviews,
`refetch_app_icon` evals the script directly.) This requires factoring the open/create
logic of `open_app` into a reusable inner `ensure_app_open(app_handle, space_id, app_id,
state) -> Result<bool, String>` so `refetch_app_icon` can trigger creation without
duplicating the ~250-line builder. `open_app` becomes a thin wrapper over it that also
does the "switch to existing if present" fast path and emits `app-woke`.

### `src-tauri/src/lib.rs`

Register the two new commands (`refetch_app_icon`, `capture_favicon_done`) in the
`invoke_handler!`.

## Frontend Changes

### `src/lib/api.ts`

Add:

```ts
export async function refetchAppIcon(spaceId: string, appId: string): Promise<string> {
  return invoke("refetch_app_icon", { spaceId, appId });
}
```

### `src/lib/components/AppDialog.svelte`

`handleRefetchFavicon` branches on `mode`:

- **`edit`** → call `refetchAppIcon(spaceId, appId!)`; on success set `icon` to the
  returned path (the `iconPreviewSrc` derived value refreshes automatically). Persisted
  on Save via the existing `editApp` call. On error, show a toast and keep the current
  icon.
- **`add`** → unchanged: `fetchSiteInfo(url)` (no webview exists yet, so authenticated
  capture isn't possible; the generic fetch is the only option).

The `loading` state already drives the button's disabled/"…" label. Because the
auto-open flow can take a few seconds, the loading indicator stays visible for the
duration of the `refetchAppIcon` promise.

## Edge Cases & Behaviour

- **App not authenticated** → opening still loads a login page; capture yields the
  login-page favicon (no worse than today). The user signs in and refetches again.
- **Page never loads / 25 s timeout** → `refetch_app_icon` returns an error; the
  frontend shows a toast and keeps the existing icon. The pending entry is cleaned up.
- **Already-open webview** → no view switch; capture runs against the loaded page
  directly (the script's `readyState`/debounce handling covers an in-flight load too).
- **Cross-origin favicon (rare)** — handled by the chosen mechanism: JS only returns
  *URLs* (never fetches bytes), so CORS is irrelevant; Rust downloads the public asset.
  If the discovered URL is itself auth-gated (very rare), `download_favicon` fails for
  it and `download_first_favicon` falls through to the next candidate / root favicon /
  Google service.
- **Icon-only** — refetch does **not** touch the app name (the user may have renamed
  it). Only the icon changes.
- **Double refetch / concurrency** — if a capture is already pending for an app, a
  second `refetch_app_icon` returns early with a "refetch already in progress" error
  rather than orphaning the first awaiter.
- **Orphaned old icon files** — pre-existing behaviour (editing an icon leaves the old
  file on disk); out of scope. Could add cleanup in a follow-up.

## File Change Summary

| File | Change |
|---|---|
| `src-tauri/src/state.rs` | +1 `pending_icon_captures` field + init |
| `src-tauri/src/commands/favicon.rs` | +`build_favicon_capture_js`, +`refetch_app_icon`, +`capture_favicon_done`, refactor `download_first_favicon` out of `try_download_favicon` |
| `src-tauri/src/commands/webviews.rs` | +capture injection in `on_page_load`; factor `ensure_app_open` out of `open_app` |
| `src-tauri/src/lib.rs` | register 2 new commands |
| `src/lib/api.ts` | +`refetchAppIcon` wrapper |
| `src/lib/components/AppDialog.svelte` | branch `handleRefetchFavicon` on `mode` |

## Testing

- **Unit (Rust):**
  - The refactored `download_first_favicon` preserves the existing priority/fallback
    behaviour (feed it a fixture URL list and assert which candidate is chosen).
  - Structural assertions on `build_favicon_capture_js`: contains the priority sort, the
    `capture_favicon_done` invoke call, the `readyState`/debounce wait, and the
    idempotency guard.
- **Manual:**
  - Add Gmail / Outlook → confirm generic icon on first add.
  - Sign in, open Edit → Re-fetch → confirm the sidebar shows the dedicated icon after
    Save.
  - Test the was-closed case (refetch with the app's webview closed/slept — should
    auto-open, load the authenticated page, capture).
  - Test the already-open case.
  - Test timeout (disconnect network mid-refetch) → toast, icon unchanged.
  - Test add-mode Re-fetch still does the generic `fetchSiteInfo`.
  - Test the restore-previous-view behaviour: with a different app active, refetch
    another app's icon → main view returns to the previously-active app after capture.

## Out of Scope

- Improving the initial (unauthenticated) add-time favicon retrieval (e.g. parsing the
  web app manifest for better unauthenticated icons).
- JS byte-fetch for auth-gated favicons (the rare edge case the chosen mechanism
  doesn't cover; can be added later if a specific app needs it).
- Cleaning up orphaned icon files on edit/refetch.
- Adding "Refetch icon" to the sidebar right-click context menu.
