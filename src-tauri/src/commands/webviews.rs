use std::fs;

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewBuilder, WebviewUrl};
use tauri::webview::NewWindowResponse;
use tauri::menu::{ContextMenu, Menu, MenuItem};

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;

const TOPBAR_HEIGHT: f64 = 48.0;

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
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .data_directory(data_dir)
        .on_navigation(|_url| true)
        .on_new_window(move |url, _features| {
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
    *active_app = Some(app_id);
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
pub fn show_app_context_menu(app_handle: AppHandle, space_id: String, app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    // Store the target so the menu-event handler knows which app was right-clicked
    {
        let mut target = state.context_menu_target.lock().map_err(|e| e.to_string())?;
        *target = Some((space_id, app_id));
    }

    let window = app_handle.get_window("main").ok_or("Main window not found")?;

    let remove_item = MenuItem::with_id(&app_handle, "ctx-remove-app", "Remove", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let menu = Menu::with_items(&app_handle, &[&remove_item])
        .map_err(|e| e.to_string())?;

    menu.popup(window).map_err(|e| e.to_string())?;

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
