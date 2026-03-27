use tauri::{AppHandle, LogicalPosition, Manager, State};
use tauri::menu::{ContextMenu, Menu, MenuItem};

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;

#[tauri::command]
pub fn list_spaces(state: State<'_, AppState>) -> Result<Vec<SpaceConfig>, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let config = state.global_config.lock().map_err(|e| e.to_string())?;
    let order = &config.general.space_order;

    if order.is_empty() {
        return Ok(spaces.clone());
    }

    let mut sorted = spaces.clone();
    sorted.sort_by_key(|s| {
        order.iter().position(|id| id == &s.space.id).unwrap_or(usize::MAX)
    });
    Ok(sorted)
}

#[tauri::command]
pub fn get_active_space(state: State<'_, AppState>) -> Result<String, String> {
    let id = state.active_space_id.lock().map_err(|e| e.to_string())?;
    Ok(id.clone())
}

#[tauri::command]
pub fn create_space(name: String, color: Option<String>, state: State<'_, AppState>) -> Result<SpaceConfig, String> {
    let id = name.to_lowercase().replace(' ', "-");
    let space = SpaceConfig {
        space: SpaceInfo {
            id: id.clone(),
            name,
            icon: "folder".to_string(),
            color: color.unwrap_or_else(|| "#4a9eff".to_string()),
            isolation: IsolationMode::default(),
        },
        apps: vec![],
    };
    storage::save_space(&space).map_err(|e| e.to_string())?;
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    spaces.push(space.clone());

    // Append to space_order
    let mut config = state.global_config.lock().map_err(|e| e.to_string())?;
    config.general.space_order.push(id);
    storage::save_global_config(&config).map_err(|e| e.to_string())?;

    Ok(space)
}

#[tauri::command]
pub fn rename_space(space_id: String, new_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    if let Some(space) = spaces.iter_mut().find(|s| s.space.id == space_id) {
        space.space.name = new_name;
        storage::save_space(space).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_space(space_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if space_id == "general" {
        return Err("Cannot delete the default General space".to_string());
    }
    storage::delete_space_file(&space_id).map_err(|e| e.to_string())?;
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    spaces.retain(|s| s.space.id != space_id);
    let mut active = state.active_space_id.lock().map_err(|e| e.to_string())?;
    if *active == space_id {
        *active = "general".to_string();
    }

    // Remove from space_order
    let mut config = state.global_config.lock().map_err(|e| e.to_string())?;
    config.general.space_order.retain(|id| id != &space_id);
    storage::save_global_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn switch_space(space_id: String, state: State<'_, AppState>) -> Result<SpaceConfig, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces.iter().find(|s| s.space.id == space_id).cloned()
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    let mut active = state.active_space_id.lock().map_err(|e| e.to_string())?;
    *active = space_id;
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = None;
    Ok(space)
}

#[tauri::command]
pub fn edit_space(space_id: String, name: Option<String>, color: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    if let Some(space) = spaces.iter_mut().find(|s| s.space.id == space_id) {
        if let Some(n) = name {
            space.space.name = n;
        }
        if let Some(c) = color {
            space.space.color = c;
        }
        storage::save_space(space).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn reorder_spaces(space_ids: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = state.global_config.lock().map_err(|e| e.to_string())?;
    config.general.space_order = space_ids;
    storage::save_global_config(&config).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_space_isolation(space_id: String, mode: IsolationMode, state: State<'_, AppState>) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    if let Some(space) = spaces.iter_mut().find(|s| s.space.id == space_id) {
        space.space.isolation = mode;
        storage::save_space(space).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn show_space_context_menu(app_handle: AppHandle, space_id: String, x: f64, y: f64, state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut target = state.space_context_menu_target.lock().map_err(|e| e.to_string())?;
        *target = Some(space_id.clone());
    }

    let window = app_handle.get_window("main").ok_or("Main window not found")?;

    let edit_item = MenuItem::with_id(&app_handle, "ctx-edit-space", "Edit Space", true, None::<&str>)
        .map_err(|e| e.to_string())?;

    let is_general = space_id == "general";
    let delete_item = MenuItem::with_id(&app_handle, "ctx-delete-space", "Delete Space", !is_general, None::<&str>)
        .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(&app_handle, &[&edit_item, &delete_item])
        .map_err(|e| e.to_string())?;

    menu.popup_at(window, LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;

    Ok(())
}
