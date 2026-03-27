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

const TOPBAR_HEIGHT: f64 = 48.0;
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

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

    // Wake from sleep if needed
    {
        let mut slept = state.slept_apps.lock().map_err(|e| e.to_string())?;
        slept.remove(&app_clone.id);
    }

    // Check if webview already exists; if so, just switch to it
    {
        let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
        if labels.contains_key(&app_clone.id) {
            drop(labels);
            return switch_to_app(app_handle, space_id, app_id, state);
        }
    }

    let data_dir = resolve_data_directory(&space_clone, &app_clone)?;
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    let window = app_handle.get_window("main").ok_or("Main window not found")?;

    let sidebar_width = {
        let config = state.global_config.lock().map_err(|e| e.to_string())?;
        config.general.sidebar_width
    };

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

    let label_for_nav = label.clone();
    let app_handle_for_nav = app_handle.clone();

    let webview_builder = WebviewBuilder::new(&label, webview_url)
        .user_agent(USER_AGENT)
        .data_directory(data_dir)
        .on_navigation(|_url| true)
        .on_new_window(move |url, features| {
            // Allow popups for OAuth/login flows — create a proper window
            // that shares the parent's web context (cookies, ITP settings)
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
            // For regular links, navigate in the same webview
            if let Some(webview) = app_handle_for_nav.get_webview(&label_for_nav) {
                let url_str = url.as_str().replace('\\', "\\\\").replace('\'', "\\'");
                let _ = webview.eval(&format!("window.location.href = '{}'", url_str));
            }
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

    window.add_child(
        webview_builder,
        LogicalPosition::new(sidebar_width as f64, TOPBAR_HEIGHT),
        LogicalSize::new(logical_width - sidebar_width as f64, logical_height - TOPBAR_HEIGHT),
    ).map_err(|e| e.to_string())?;

    // On Linux: disable ITP and set cookie policy to accept all cookies
    // so that Cloudflare challenges and similar cross-origin flows work properly
    #[cfg(target_os = "linux")]
    {
        if let Some(webview) = app_handle.get_webview(&label) {
            let _ = webview.with_webview(|platform_webview| {
                use webkit2gtk::{WebViewExt, WebsiteDataManagerExt, CookieManagerExt};
                let wk_webview = platform_webview.inner();
                if let Some(data_manager) = wk_webview.website_data_manager() {
                    data_manager.set_itp_enabled(false);
                    if let Some(cookie_manager) = data_manager.cookie_manager() {
                        cookie_manager.set_accept_policy(webkit2gtk::CookieAcceptPolicy::Always);
                    }
                }
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

    let mut last_active = state.last_active.lock().map_err(|e| e.to_string())?;
    last_active.insert(app_clone.id.clone(), Instant::now());

    let _ = app_handle.emit("app-woke", &app_clone.id);

    Ok(())
}

#[tauri::command]
pub fn switch_to_app(app_handle: AppHandle, _space_id: String, app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;

    // Hide all app webviews
    for (_, label) in labels.iter() {
        if let Some(webview) = app_handle.get_webview(label) {
            let _ = webview.hide();
        }
    }

    // Show the target app webview
    if let Some(label) = labels.get(&app_id) {
        if let Some(webview) = app_handle.get_webview(label) {
            webview.show().map_err(|e| e.to_string())?;
        }
    }

    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = Some(app_id.clone());

    let mut last_active = state.last_active.lock().map_err(|e| e.to_string())?;
    last_active.insert(app_id, Instant::now());

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
    }
    // Clean up sleep tracking
    let mut last_active = state.last_active.lock().map_err(|e| e.to_string())?;
    last_active.remove(&app_id);
    let mut slept = state.slept_apps.lock().map_err(|e| e.to_string())?;
    slept.remove(&app_id);
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
    Ok(())
}

#[tauri::command]
pub fn get_active_app(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let active = state.active_app_id.lock().map_err(|e| e.to_string())?;
    Ok(active.clone())
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
    Ok(())
}

#[tauri::command]
pub fn get_slept_apps(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let slept = state.slept_apps.lock().map_err(|e| e.to_string())?;
    Ok(slept.iter().cloned().collect())
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
