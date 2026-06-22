use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewBuilder, WebviewUrl, WebviewWindowBuilder};
use tauri::webview::NewWindowResponse;
use tauri::menu::{ContextMenu, Menu, MenuItem};

static POPUP_COUNTER: AtomicU32 = AtomicU32::new(0);

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;
use crate::commands::favicon::build_favicon_capture_js;

const TOPBAR_HEIGHT: f64 = 48.0;
// NOTE: We present as Safari (real WebKit), NOT a spoofed Chrome. Microsoft
// negotiates its auth flow from the User-Agent: a Chromium UA makes it serve
// the EAR (Encrypted Authentication Response) flow, which relies on
// Chromium-specific crypto/storage behavior our WebKitGTK engine can't
// complete — causing the SharePoint/Azure-AD login to loop and fail with
// "We couldn't sign you in." A Safari UA matches our actual engine, so the
// served flow is WebKit-compatible.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15";

const MEDIA_GUARD_JS: &str = r#"
(function() {
  if (window.__webapps_media_guard_installed) return;
  window.__webapps_media_guard_installed = true;

  if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) return;

  var realGUM = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
  var activeStreams = new Set();

  navigator.mediaDevices.getUserMedia = async function(constraints) {
    constraints = constraints || {};
    var wantsVideo = !!constraints.video;
    var wantsAudio = !!constraints.audio;

    try {
      var perms = await window.__TAURI_INTERNALS__.invoke('check_app_media_permissions');
      if ((wantsVideo && perms.camera === 'block') ||
          (wantsAudio && perms.microphone === 'block')) {
        throw new DOMException('Permission denied by user', 'NotAllowedError');
      }
    } catch (e) {
      if (e && e.name === 'NotAllowedError') throw e;
      // If the permission check fails for any other reason, fall through to real GUM.
    }

    var stream = await realGUM(constraints);
    activeStreams.add(stream);
    return stream;
  };

  document.addEventListener('__webapps_revoke_media', function(e) {
    var kind = e.detail && e.detail.kind;
    activeStreams.forEach(function(stream) {
      stream.getTracks().forEach(function(track) {
        if ((kind === 'camera' && track.kind === 'video') ||
            (kind === 'microphone' && track.kind === 'audio')) {
          track.stop();
        }
      });
    });
  });
})();
"#;

/// The registrable domain (eTLD+1) of `host`, e.g. `docs.google.com` -> `google.com`.
/// Returns `None` for single-label hosts with no eTLD+1 (e.g. `localhost`).
fn registrable_domain(host: &str) -> Option<&str> {
    psl::domain_str(host)
}

/// Build the click interceptor injected into each app webview, run in the
/// capture phase (before the page reacts to the click):
///
/// - Third-party links (different registrable domain) go to the system browser.
/// - Same-site links that target a new window/tab open as an in-app popup.
///   WebKitGTK does not reliably open same-site `target="_blank"` anchors, and
///   apps like Google Drive open docs by synthesizing such an anchor and
///   `.click()`-ing it — a non-user-gesture the engine's popup blocker kills.
///   Handling it here in capture (while `defaultPrevented` is still false) lets
///   us route it to a popup ourselves.
/// - Same-site same-tab links are left alone to navigate normally.
///
/// `base_domain` is the app's registrable domain (e.g. `google.com`); when
/// `None` (IP/localhost apps) we fall back to an exact-hostname comparison.
fn build_link_interceptor_js(app_id: &str, base_domain: Option<&str>) -> String {
    // serde_json renders a safely-quoted JS string literal (or `null`).
    let app_id_literal = serde_json::to_string(app_id).unwrap_or_else(|_| "\"\"".to_string());
    let base_domain_literal = match base_domain {
        Some(d) => serde_json::to_string(d).unwrap_or_else(|_| "null".to_string()),
        None => "null".to_string(),
    };
    format!(
        r#"
(function() {{
  var APP_ID = {app_id_literal};
  var baseHostname = window.location.hostname;
  var baseDomain = {base_domain_literal};

  function invoke(cmd, args) {{
    if (window.__TAURI_INTERNALS__) {{
      try {{ window.__TAURI_INTERNALS__.invoke(cmd, args); }} catch (e) {{}}
    }}
  }}

  function isSameSite(host) {{
    if (baseDomain) {{
      return host === baseDomain || host.endsWith('.' + baseDomain);
    }}
    return host === baseHostname;
  }}

  function findAnchor(node) {{
    while (node && node.tagName !== 'A') {{ node = node.parentElement; }}
    return node;
  }}

  function isHttp(el) {{
    return el.protocol === 'http:' || el.protocol === 'https:';
  }}

  // True when the anchor targets a NEW window/tab (target=_blank or a named
  // target), as opposed to navigating the current frame.
  function opensNewWindow(el) {{
    var t = (el.target || '').toLowerCase();
    return t === '_blank' || (t && t !== '_self' && t !== '_top' && t !== '_parent');
  }}

  document.addEventListener('click', function(e) {{
    var el = findAnchor(e.target);
    if (!el || !el.href || !isHttp(el)) return;

    if (!isSameSite(el.hostname)) {{
      // Third-party: hand off to the system browser.
      e.preventDefault();
      e.stopPropagation();
      invoke('open_in_browser', {{ url: el.href }});
      return;
    }}

    if (opensNewWindow(el)) {{
      // Same-site new-window link: open it in an in-app popup.
      e.preventDefault();
      e.stopPropagation();
      invoke('open_blank_popup', {{ appId: APP_ID, url: el.href }});
    }}
    // Same-site same-tab link: let it navigate normally.
  }}, true);
}})();
"#
    )
}

/// Override `window.open` so Google Drive's "blank-first" popup pattern works.
///
/// Drive opens a doc by calling `window.open()` with NO url during the click
/// gesture (to grab a handle synchronously), then later sets `win.location` to
/// the doc URL. On WebKitGTK that blank `window.open` produces a navigation
/// action with no request URI, which wry rejects before our `on_new_window`
/// handler ever runs — so `window.open()` returns null and Drive reports
/// "Browser blocked opening a window".
///
/// We intercept in JS: for a real URL we defer to the native `window.open`
/// (whose new-window handler routes SSO/same-site/external correctly and keeps
/// a live `window.opener`); for the blank case we return a Proxy that emulates
/// just enough of a Window so the page can set `location`, and forward that URL
/// to the `open_blank_popup` command, which opens it in-app or in the browser.
fn build_window_open_override_js(app_id: &str, base_domain: Option<&str>) -> String {
    let app_id_literal = serde_json::to_string(app_id).unwrap_or_else(|_| "\"\"".to_string());
    let base_domain_literal = match base_domain {
        Some(d) => serde_json::to_string(d).unwrap_or_else(|_| "null".to_string()),
        None => "null".to_string(),
    };
    format!(
        r#"
(function() {{
  if (window.__webapps_open_override) return;
  window.__webapps_open_override = true;

  var APP_ID = {app_id_literal};
  var BASE_DOMAIN = {base_domain_literal};
  var realOpen = window.open ? window.open.bind(window) : null;

  function invoke(cmd, args) {{
    if (window.__TAURI_INTERNALS__) {{
      try {{ return window.__TAURI_INTERNALS__.invoke(cmd, args); }} catch (e) {{}}
    }}
  }}

  function openInApp(u) {{
    if (!u || u === 'about:blank') return;
    invoke('open_blank_popup', {{ appId: APP_ID, url: u }});
  }}

  // A minimal Window stand-in. Drive sets `.location` (string), `.location.href`,
  // or calls `.location.assign(...)`; any of those triggers the in-app popup.
  function makeProxy() {{
    var opened = false;
    function go(u) {{ if (opened || !u) return; opened = true; openInApp(String(u)); }}

    var loc = {{
      assign: function(u) {{ go(u); }},
      replace: function(u) {{ go(u); }},
      reload: function() {{}},
      toString: function() {{ return 'about:blank'; }}
    }};
    var locProxy = new Proxy(loc, {{
      get: function(t, p) {{ if (p === 'href') return 'about:blank'; return t[p]; }},
      set: function(t, p, v) {{ if (p === 'href') go(v); t[p] = v; return true; }}
    }});

    var win = {{
      closed: false,
      focus: function() {{}},
      blur: function() {{}},
      close: function() {{ win.closed = true; }},
      postMessage: function() {{}},
      document: {{ write: function() {{}}, writeln: function() {{}}, open: function() {{}}, close: function() {{}} }}
    }};
    return new Proxy(win, {{
      get: function(t, p) {{
        if (p === 'location') return locProxy;
        if (p in t) return t[p];
        return undefined;
      }},
      set: function(t, p, v) {{
        if (p === 'location') {{ go(typeof v === 'string' ? v : (v && v.href)); return true; }}
        t[p] = v; return true;
      }}
    }});
  }}

  window.open = function(url, target, features) {{
    var resolved = '';
    try {{ resolved = url ? new URL(url, window.location.href).href : ''; }}
    catch (e) {{ resolved = url ? String(url) : ''; }}

    if (resolved && resolved !== 'about:blank') {{
      // Real URL: the native path already routes correctly and preserves opener.
      if (realOpen) return realOpen(url, target, features);
      openInApp(resolved);
      return makeProxy();
    }}
    // Blank-first popup: the engine blocks this, so emulate it ourselves.
    return makeProxy();
  }};
}})();
"#
    )
}

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

fn cleanup_media_state(app_handle: &AppHandle, app_id: &str, state: &AppState) {
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::PermissionRequestExt;
        let had_pending = if let Ok(mut pending) = state.pending_media_requests.lock() {
            if let Some(p) = pending.remove(app_id) {
                p.request.deny();
                true
            } else {
                false
            }
        } else {
            false
        };
        if had_pending {
            let _ = app_handle.emit("media-permission-cancelled", app_id);
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

fn resolve_data_directory(space: &SpaceConfig, app: &AppConfig) -> Result<std::path::PathBuf, String> {
    let use_per_app = space.space.isolation == IsolationMode::PerApp || app.isolation_override;
    if use_per_app {
        storage::webview_data_dir(&space.space.id, Some(&app.id))
    } else {
        storage::webview_data_dir(&space.space.id, None)
    }
    .map_err(|e| e.to_string())
}

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

    let data_dir = resolve_data_directory(&space_clone, &app_clone)?;
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    let window = app_handle.get_window("main").ok_or("Main window not found")?;

    let (sidebar_width, sidebar_visible) = {
        let config = state.global_config.lock().map_err(|e| e.to_string())?;
        (config.general.sidebar_width, *state.sidebar_visible.lock().map_err(|e| e.to_string())?)
    };
    let sidebar_x = if sidebar_visible { sidebar_width as f64 } else { 0.0 };

    let window_size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let logical_width = window_size.width as f64 / scale;
    let logical_height = window_size.height as f64 / scale;

    // Hide all existing app webviews before showing the new one
    {
        let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
        for (_, lbl) in labels.iter() {
            if let Some(webview) = app_handle.get_webview(lbl) {
                let _ = webview.hide();
            }
        }
    }

    let webview_url = WebviewUrl::External(app_clone.url.parse().map_err(|e: url::ParseError| e.to_string())?);

    let app_id_for_title = app_clone.id.clone();
    let app_handle_for_title = app_handle.clone();

    let app_id_for_capture = app_clone.id.clone();
    let app_handle_for_capture = app_handle.clone();

    let label_for_nav = label.clone();
    let app_handle_for_nav = app_handle.clone();

    let base_url_host = url::Url::parse(&app_clone.url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()));

    // Registrable domain (eTLD+1) of the app, e.g. drive.google.com -> google.com.
    // Used to keep same-site `window.open` targets (docs.google.com) in-app while
    // still sending genuinely third-party links to the system browser.
    let base_registrable_domain = base_url_host
        .as_deref()
        .and_then(|h| registrable_domain(h).map(|d| d.to_string()));

    // Pre-render the click interceptor with the app's registrable domain baked in,
    // so the JS and the on_new_window path agree on what counts as "same site".
    let link_interceptor_js =
        build_link_interceptor_js(&app_clone.id, base_registrable_domain.as_deref());

    // Pre-render the window.open override (handles Drive's blank-first popups,
    // which the engine blocks before on_new_window can run).
    let window_open_override_js =
        build_window_open_override_js(&app_clone.id, base_registrable_domain.as_deref());

    // SSO/OAuth popups must live in the SAME WebKit web context as the app
    // webview that opened them. tauri-runtime-wry keys web contexts by
    // data_directory, so giving the popup the parent's data_directory makes
    // them share one WebKitWebContext. That shared context is what gives the
    // popup the parent's cookies/session AND a live `window.opener` +
    // postMessage channel back to the opener. Without it, Microsoft auth lands
    // its session in a separate cookie jar ("we could not authenticate your
    // user") and Google Identity Services hangs waiting on a dead opener.
    let popup_data_dir = data_dir.clone();

    let webview_builder = WebviewBuilder::new(&label, webview_url)
        .user_agent(USER_AGENT)
        .data_directory(data_dir)
        .on_navigation(|_url| true)
        .on_new_window(move |url, features| {
            let host = url.host_str().unwrap_or("");

            // Open `url` in an in-app popup window that shares the app's WebKit web
            // context (cookies + live window.opener) via the parent's data_directory.
            let make_popup = |features| {
                let popup_id = POPUP_COUNTER.fetch_add(1, Ordering::Relaxed);
                let popup_label = format!("popup-{}", popup_id);
                match WebviewWindowBuilder::new(
                    &app_handle_for_nav,
                    &popup_label,
                    WebviewUrl::External("about:blank".parse().unwrap()),
                )
                .window_features(features)
                .user_agent(USER_AGENT)
                .data_directory(popup_data_dir.clone())
                .inner_size(500.0, 700.0)
                .title(url.as_str())
                .build()
                {
                    Ok(window) => NewWindowResponse::Create { window },
                    Err(_) => NewWindowResponse::Allow,
                }
            };

            // 0. Blank / opener-relative window (empty host). Google Drive opens
            //    docs by calling window.open('') during the click gesture to grab a
            //    handle, then setting win.location to the doc URL. Such a window
            //    inherits the opener's origin, so it's same-site by definition —
            //    keep it in-app. Denying it instead makes window.open() return null,
            //    which Drive surfaces as "Browser blocked opening a window".
            if host.is_empty() {
                return make_popup(features);
            }

            // 1. Cross-domain SSO/OAuth providers: always in-app popups. These live
            //    on a different registrable domain than the app (e.g.
            //    login.microsoftonline.com opened from a *.sharepoint.com app), so
            //    the registrable-domain rule below would miss them.
            if is_sso_host(host) {
                return make_popup(features);
            }

            // 2. Exact same host: navigate the current webview in place rather than
            //    spawning a popup (preserves the single-window feel for in-app links).
            if base_url_host.as_deref() == Some(host) {
                if let Some(webview) = app_handle_for_nav.get_webview(&label_for_nav) {
                    let url_str = url.as_str().replace('\\', "\\\\").replace('\'', "\\'");
                    let _ = webview.eval(&format!("window.location.href = '{}'", url_str));
                }
                return NewWindowResponse::Deny;
            }

            // 3. Same registrable domain, different host (e.g. docs.google.com opened
            //    from drive.google.com): keep it in-app as a popup.
            let same_site = match (&base_registrable_domain, registrable_domain(host)) {
                (Some(base), Some(new)) => base.as_str() == new,
                _ => false,
            };
            if same_site {
                return make_popup(features);
            }

            // 4. Genuinely third-party: hand off to the system browser.
            let _ = open::that(url.as_str());
            NewWindowResponse::Deny
        })
        .on_document_title_changed(move |_webview, title| {
            let count = parse_badge_count(&title);
            let _ = app_handle_for_title.emit(
                "title-changed",
                serde_json::json!({
                    "app_id": app_id_for_title,
                    "title": title,
                    "badge": count
                }),
            );
        });

    let webview_builder = webview_builder
        .on_page_load(move |webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let _ = webview.eval(MEDIA_GUARD_JS);
                let _ = webview.eval(&window_open_override_js);
                let _ = webview.eval(crate::commands::shortcuts::build_shortcut_listener_js());
            }
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = webview.eval(&link_interceptor_js);
                // If an icon refetch is pending for this app, inject the
                // capture script (it waits for load + debounce, then reports
                // the authenticated DOM's favicon URLs back to Rust).
                let state = app_handle_for_capture.state::<AppState>();
                let guard = state.pending_icon_captures.lock();
                if let Ok(guard) = guard {
                    if guard.contains_key(&app_id_for_capture) {
                        let _ = webview.eval(&build_favicon_capture_js(&app_id_for_capture));
                    }
                }
            }
        });

    window.add_child(
        webview_builder,
        LogicalPosition::new(sidebar_x, TOPBAR_HEIGHT),
        LogicalSize::new(logical_width - sidebar_x, logical_height - TOPBAR_HEIGHT),
    ).map_err(|e| e.to_string())?;

    // On Linux: disable ITP and set cookie policy to accept all cookies
    // so that Cloudflare challenges and similar cross-origin flows work properly.
    // Also connect the WebKit permission-request signal to handle camera/microphone.
    #[cfg(target_os = "linux")]
    {
        let app_handle_for_perm = app_handle.clone();
        let space_id_for_perm = space_id.to_string();
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
                    use webkit2gtk::glib::Cast;
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

                // Capture-state notifications
                let app_handle_cap = app_handle_for_perm.clone();
                let app_id_cap = app_id_for_perm.clone();
                wk_webview.connect_camera_capture_state_notify(move |wv| {
                    use webkit2gtk::WebViewExt;
                    let active = !matches!(
                        wv.camera_capture_state(),
                        webkit2gtk::MediaCaptureState::None
                    );
                    update_capture_state(&app_handle_cap, &app_id_cap, "camera", active);
                });

                let app_handle_cap2 = app_handle_for_perm.clone();
                let app_id_cap2 = app_id_for_perm.clone();
                wk_webview.connect_microphone_capture_state_notify(move |wv| {
                    use webkit2gtk::WebViewExt;
                    let active = !matches!(
                        wv.microphone_capture_state(),
                        webkit2gtk::MediaCaptureState::None
                    );
                    update_capture_state(&app_handle_cap2, &app_id_cap2, "microphone", active);
                });
            });
        }
    }

    // On Linux: reparent app webview from vbox into the inner horizontal box
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::*;
        let vbox = window.default_vbox().map_err(|e| e.to_string())?;
        let children = vbox.children();
        // Layout: [topbar, inner_hbox] — new app widget is appended last
        let app_widget = children.last().cloned();
        let inner_hbox_widget = children.get(1).cloned();

        if let (Some(app_w), Some(inner_w)) = (app_widget, inner_hbox_widget) {
            if let Some(inner_hbox) = inner_w.downcast_ref::<gtk::Box>() {
                vbox.remove(&app_w);
                inner_hbox.pack_start(&app_w, true, true, 0);
            }
        }
    }

    let mut labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    labels.insert(app_clone.id.clone(), label);

    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = Some(app_clone.id.clone());
    let _ = app_handle.emit("active-app-changed", Some(app_clone.id.clone()));

    let mut last_active = state.last_active.lock().map_err(|e| e.to_string())?;
    last_active.insert(app_clone.id.clone(), Instant::now());

    let _ = app_handle.emit("app-woke", &app_clone.id);

    // Focus the newly-created app webview so keyboard shortcuts chain naturally
    // (otherwise focus stays on whichever webview triggered the open).
    if let Some(wv) = app_handle.get_webview(&format!("app-{}", app_clone.id)) {
        let _ = wv.set_focus();
    }

    Ok(())
}

#[tauri::command]
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
                // Move keyboard focus onto the newly-shown app so shortcut
                // chaining works (Ctrl+2 → Ctrl+3) without a manual click.
                let _ = webview.set_focus();
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

#[tauri::command]
pub fn close_app(app_handle: AppHandle, app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    if let Some(label) = labels.remove(&app_id) {
        if let Some(webview) = app_handle.get_webview(&label) {
            webview.close().map_err(|e| e.to_string())?;
        }
    }
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    if active_app.as_deref() == Some(&app_id) {
        *active_app = None;
        let _ = app_handle.emit("active-app-changed", None::<String>);
    }
    // Clean up sleep tracking
    let mut last_active = state.last_active.lock().map_err(|e| e.to_string())?;
    last_active.remove(&app_id);
    let mut slept = state.slept_apps.lock().map_err(|e| e.to_string())?;
    slept.remove(&app_id);
    cleanup_media_state(&app_handle, &app_id, &state);
    Ok(())
}

#[tauri::command]
pub fn hide_all_app_webviews(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    for (_, label) in labels.iter() {
        if let Some(webview) = app_handle.get_webview(label) {
            let _ = webview.hide();
        }
    }
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = None;
    let _ = app_handle.emit("active-app-changed", None::<String>);
    Ok(())
}

#[tauri::command]
pub fn get_active_app(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let active = state.active_app_id.lock().map_err(|e| e.to_string())?;
    Ok(active.clone())
}

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

/// Flip sidebar visibility, persist it, hide/show the sidebar webview, and
/// resize the active app webview to fill the new area.
pub fn toggle_sidebar_inner(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    let new_visible = {
        let mut visible = state.sidebar_visible.lock().map_err(|e| e.to_string())?;
        *visible = !*visible;
        *visible
    };

    // Persist (non-fatal: in-memory state is already flipped). Keep
    // global_config in sync with the AppState flag so the saved value matches.
    {
        let mut cfg = state.global_config.lock().map_err(|e| e.to_string())?;
        cfg.general.sidebar_visible = new_visible;
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

#[tauri::command]
pub fn show_app_context_menu(app_handle: AppHandle, space_id: String, app_id: String, x: f64, y: f64, state: State<'_, AppState>) -> Result<(), String> {
    // Store the target so the menu-event handler knows which app was right-clicked
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

    menu.popup_at(window, LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn webview_go_back(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active = state.active_app_id.lock().map_err(|e| e.to_string())?;
    let app_id = active.as_ref().ok_or("No active app")?;
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    let label = labels.get(app_id).ok_or("Webview not found")?;
    let webview = app_handle.get_webview(label).ok_or("Webview not found")?;
    webview.eval("window.history.back()").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn webview_reload(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active = state.active_app_id.lock().map_err(|e| e.to_string())?;
    let app_id = active.as_ref().ok_or("No active app")?;
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    let label = labels.get(app_id).ok_or("Webview not found")?;
    let webview = app_handle.get_webview(label).ok_or("Webview not found")?;
    webview.eval("window.location.reload()").map_err(|e| e.to_string())
}

/// Destroy a webview to free memory, marking it as slept so the frontend knows it can be reopened.
pub fn sleep_app_inner(app_handle: &AppHandle, app_id: &str, state: &AppState) -> Result<(), String> {
    let mut labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    if let Some(label) = labels.remove(app_id) {
        if let Some(webview) = app_handle.get_webview(&label) {
            webview.close().map_err(|e| e.to_string())?;
        }
    }
    let mut slept = state.slept_apps.lock().map_err(|e| e.to_string())?;
    slept.insert(app_id.to_string());
    let _ = app_handle.emit("app-slept", app_id);
    cleanup_media_state(app_handle, app_id, state);
    Ok(())
}

#[tauri::command]
pub fn get_slept_apps(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let slept = state.slept_apps.lock().map_err(|e| e.to_string())?;
    Ok(slept.iter().cloned().collect())
}

#[tauri::command]
pub fn open_in_browser(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

/// True for the cross-domain SSO/OAuth hosts we always keep as in-app popups.
fn is_sso_host(host: &str) -> bool {
    host == "accounts.google.com"
        || host == "appleid.apple.com"
        || host == "login.microsoftonline.com"
        || host == "github.com"
        || host.ends_with(".github.com")
}

/// Open a URL that a page requested via the blank-first `window.open` pattern
/// (see [`build_window_open_override_js`]). Applies the same routing as the
/// native new-window handler: SSO and same-registrable-domain targets open as an
/// in-app popup sharing the app's web context; everything else goes to the
/// system browser.
#[tauri::command]
pub fn open_blank_popup(
    app_handle: AppHandle,
    app_id: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (space_clone, app_clone) = {
        let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
        spaces
            .iter()
            .find_map(|s| {
                s.apps
                    .iter()
                    .find(|a| a.id == app_id)
                    .map(|a| (s.clone(), a.clone()))
            })
            .ok_or_else(|| format!("App '{}' not found", app_id))?
    };

    let target = url::Url::parse(&url).map_err(|e| e.to_string())?;
    let host = target.host_str().unwrap_or("");

    let base_host = url::Url::parse(&app_clone.url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()));
    let base_registrable = base_host
        .as_deref()
        .and_then(|h| registrable_domain(h).map(|d| d.to_string()));

    let same_site = base_host.as_deref() == Some(host)
        || match (&base_registrable, registrable_domain(host)) {
            (Some(b), Some(n)) => b.as_str() == n,
            _ => false,
        };

    if !is_sso_host(host) && !same_site {
        return open::that(&url).map_err(|e| e.to_string());
    }

    let data_dir = resolve_data_directory(&space_clone, &app_clone)?;
    let popup_id = POPUP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let popup_label = format!("popup-{}", popup_id);

    // Build on about:blank, then (on Linux) strip wry's injected user scripts and
    // navigate to the real URL — see `finalize_popup` for why. On other platforms
    // there's no API to strip the scripts, so just load the URL directly.
    let blank = "about:blank"
        .parse()
        .map_err(|e: url::ParseError| e.to_string())?;
    let popup = WebviewWindowBuilder::new(&app_handle, &popup_label, WebviewUrl::External(blank))
        .user_agent(USER_AGENT)
        .data_directory(data_dir)
        .inner_size(900.0, 720.0)
        .title(&url)
        .build()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    finalize_popup(&popup, &url);
    #[cfg(not(target_os = "linux"))]
    {
        let js = format!("location.replace({})", serde_json::to_string(&url).unwrap());
        let _ = popup.eval(&js);
    }
    Ok(())
}

/// Finish setting up an in-app popup webview on Linux/WebKitGTK.
///
/// wry injects `window.ipc` as a non-configurable, non-writable, non-enumerable
/// property (its IPC bridge). Google Sheets and Excel Online declare a global
/// `function ipc`, which throws a `TypeError` against that locked property and
/// aborts their bootstrap ("Problema al cargar" / clear-cache prompt). wry adds
/// that script before any of ours, so we can't fix it from JS. Popups are plain
/// web content that never call the Tauri IPC, so we remove all of wry's injected
/// user scripts here and only then navigate to the real URL — so the offending
/// `ipc` definition never runs for the document load. The shared web context
/// (cookies/session) is unaffected, since it lives in the data directory, not in
/// these scripts.
#[cfg(target_os = "linux")]
fn finalize_popup(window: &tauri::WebviewWindow, url: &str) {
    let url = url.to_string();
    let _ = window.with_webview(move |platform_webview| {
        use webkit2gtk::{
            CookieManagerExt, UserContentManagerExt, WebViewExt, WebsiteDataManagerExt,
        };
        let wk = platform_webview.inner();
        if let Some(ucm) = wk.user_content_manager() {
            ucm.remove_all_scripts();
        }
        if let Some(dm) = wk.website_data_manager() {
            dm.set_itp_enabled(false);
            if let Some(cm) = dm.cookie_manager() {
                cm.set_accept_policy(webkit2gtk::CookieAcceptPolicy::Always);
            }
        }
        wk.load_uri(&url);
    });
}

#[tauri::command]
pub fn eval_in_app(app_handle: AppHandle, app_id: String, script: String, state: State<'_, AppState>) -> Result<(), String> {
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    let label = labels.get(&app_id).ok_or("Webview not found")?;
    let webview = app_handle.get_webview(label).ok_or("Webview not found")?;
    webview.eval(&script).map_err(|e| e.to_string())
}

fn parse_badge_count(title: &str) -> u32 {
    if let Some(start) = title.find('(') {
        if let Some(end) = title[start..].find(')') {
            let inner = &title[start + 1..start + end];
            let cleaned = inner.trim_end_matches('+');
            if let Ok(n) = cleaned.parse::<u32>() {
                return n;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns whether `host` should be treated as same-site as an app whose
    /// host is `base_host`. Mirrors the registrable-domain decision used in the
    /// `on_new_window` closure (step 3).
    fn same_site(base_host: &str, host: &str) -> bool {
        let base = registrable_domain(base_host);
        match (base, registrable_domain(host)) {
            (Some(b), Some(n)) => b == n,
            _ => false,
        }
    }

    #[test]
    fn registrable_domain_collapses_subdomains() {
        assert_eq!(registrable_domain("docs.google.com"), Some("google.com"));
        assert_eq!(registrable_domain("drive.google.com"), Some("google.com"));
        assert_eq!(registrable_domain("google.com"), Some("google.com"));
    }

    #[test]
    fn registrable_domain_handles_multipart_suffixes() {
        // A naive last-two-labels heuristic would wrongly say "co.uk" here.
        assert_eq!(registrable_domain("www.bbc.co.uk"), Some("bbc.co.uk"));
        assert_eq!(registrable_domain("shop.example.com.au"), Some("example.com.au"));
    }

    #[test]
    fn registrable_domain_none_for_single_label_host() {
        // Single-label hosts have no eTLD+1, so we fall back to exact-host matching.
        assert_eq!(registrable_domain("localhost"), None);
    }

    #[test]
    fn same_site_keeps_sibling_subdomains_together() {
        // The motivating case: Google Drive opening a Google Doc.
        assert!(same_site("drive.google.com", "docs.google.com"));
        assert!(same_site("www.bbc.co.uk", "news.bbc.co.uk"));
    }

    #[test]
    fn same_site_rejects_third_parties_and_lookalikes() {
        assert!(!same_site("drive.google.com", "dropbox.com"));
        // Multi-part suffix means these are NOT the same registrable domain.
        assert!(!same_site("foo.github.io", "bar.github.io"));
        // A lookalike host on a different registrable domain.
        assert!(!same_site("drive.google.com", "google.com.attacker.com"));
    }

    #[test]
    fn link_interceptor_bakes_in_registrable_domain() {
        let js = build_link_interceptor_js("app-1", Some("google.com"));
        assert!(js.contains(r#"var baseDomain = "google.com";"#));
        assert!(js.contains(r#"var APP_ID = "app-1";"#));
        // The endsWith guard is what keeps subdomains same-site.
        assert!(js.contains("host.endsWith('.' + baseDomain)"));
    }

    #[test]
    fn link_interceptor_falls_back_to_exact_host_when_no_domain() {
        let js = build_link_interceptor_js("app-1", None);
        assert!(js.contains("var baseDomain = null;"));
        assert!(js.contains("host === baseHostname"));
    }
}
