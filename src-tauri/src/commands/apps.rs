use tauri::State;
use uuid::Uuid;

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;

#[tauri::command]
pub fn add_app(space_id: String, name: String, url: String, icon: Option<String>, state: State<'_, AppState>) -> Result<AppConfig, String> {
    let app = AppConfig {
        id: Uuid::new_v4().to_string(),
        name,
        url,
        icon: icon.unwrap_or_else(|| "auto".to_string()),
        isolation_override: false,
        permissions: AppPermissions::default(),
    };
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces.iter_mut().find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    space.apps.push(app.clone());
    storage::save_space(space).map_err(|e| e.to_string())?;
    Ok(app)
}

#[tauri::command]
pub fn remove_app(space_id: String, app_id: String, delete_data: bool, state: State<'_, AppState>) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces.iter_mut().find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    space.apps.retain(|a| a.id != app_id);
    storage::save_space(space).map_err(|e| e.to_string())?;
    if delete_data {
        let data_dir = storage::webview_data_dir(&space_id, Some(&app_id)).map_err(|e| e.to_string())?;
        if data_dir.exists() {
            std::fs::remove_dir_all(&data_dir).map_err(|e| e.to_string())?;
        }
    }
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    if active_app.as_deref() == Some(&app_id) {
        *active_app = None;
    }
    Ok(())
}

#[tauri::command]
pub fn edit_app(
    space_id: String,
    app_id: String,
    name: Option<String>,
    url: Option<String>,
    icon: Option<String>,
    isolation_override: Option<bool>,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces.iter_mut().find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    let app = space.apps.iter_mut().find(|a| a.id == app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;
    if let Some(n) = name {
        app.name = n;
    }
    if let Some(u) = url {
        app.url = u;
    }
    if let Some(i) = icon {
        app.icon = i;
    }
    if let Some(iso) = isolation_override {
        app.isolation_override = iso;
    }
    let result = app.clone();
    storage::save_space(space).map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
pub fn reorder_apps(space_id: String, app_ids: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces.iter_mut().find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    let mut reordered = Vec::new();
    for id in &app_ids {
        if let Some(app) = space.apps.iter().find(|a| &a.id == id) {
            reordered.push(app.clone());
        }
    }
    space.apps = reordered;
    storage::save_space(space).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_apps_for_space(space_id: String, state: State<'_, AppState>) -> Result<Vec<AppConfig>, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces.iter().find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    Ok(space.apps.clone())
}
