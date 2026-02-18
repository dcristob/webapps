use std::fs;

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewBuilder, WebviewUrl};

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;

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

    let webview_url = WebviewUrl::External(app_clone.url.parse().map_err(|e: url::ParseError| e.to_string())?);

    let app_id_for_title = app_clone.id.clone();
    let app_handle_for_title = app_handle.clone();

    let webview_builder = WebviewBuilder::new(&label, webview_url)
        .auto_resize()
        .data_directory(data_dir)
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
        LogicalPosition::new(sidebar_width as f64, 0.0),
        LogicalSize::new(logical_width - sidebar_width as f64, logical_height),
    ).map_err(|e| e.to_string())?;

    let mut labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    labels.insert(app_clone.id.clone(), label);

    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = Some(app_clone.id.clone());

    Ok(())
}

#[tauri::command]
pub fn switch_to_app(app_handle: AppHandle, _space_id: String, app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;

    for (_, label) in labels.iter() {
        if let Some(webview) = app_handle.get_webview(label) {
            webview.set_size(LogicalSize::new(0.0, 0.0)).map_err(|e| e.to_string())?;
        }
    }

    if let Some(label) = labels.get(&app_id) {
        if let Some(webview) = app_handle.get_webview(label) {
            let window = app_handle.get_window("main").ok_or("Main window not found")?;
            let sidebar_width = {
                let config = state.global_config.lock().map_err(|e| e.to_string())?;
                config.general.sidebar_width
            };
            let window_size = window.inner_size().map_err(|e| e.to_string())?;
            let scale = window.scale_factor().map_err(|e| e.to_string())?;
            let logical_width = window_size.width as f64 / scale;
            let logical_height = window_size.height as f64 / scale;

            webview.set_position(LogicalPosition::new(sidebar_width as f64, 0.0)).map_err(|e| e.to_string())?;
            webview.set_size(LogicalSize::new(logical_width - sidebar_width as f64, logical_height)).map_err(|e| e.to_string())?;
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
            webview.set_size(LogicalSize::new(0.0, 0.0)).map_err(|e| e.to_string())?;
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
