use tauri::State;

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;

#[tauri::command]
pub fn list_spaces(state: State<'_, AppState>) -> Result<Vec<SpaceConfig>, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    Ok(spaces.clone())
}

#[tauri::command]
pub fn get_active_space(state: State<'_, AppState>) -> Result<String, String> {
    let id = state.active_space_id.lock().map_err(|e| e.to_string())?;
    Ok(id.clone())
}

#[tauri::command]
pub fn create_space(name: String, state: State<'_, AppState>) -> Result<SpaceConfig, String> {
    let id = name.to_lowercase().replace(' ', "-");
    let space = SpaceConfig {
        space: SpaceInfo {
            id: id.clone(),
            name,
            icon: "folder".to_string(),
            isolation: IsolationMode::default(),
        },
        apps: vec![],
    };
    storage::save_space(&space).map_err(|e| e.to_string())?;
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    spaces.push(space.clone());
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
pub fn set_space_isolation(space_id: String, mode: IsolationMode, state: State<'_, AppState>) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    if let Some(space) = spaces.iter_mut().find(|s| s.space.id == space_id) {
        space.space.isolation = mode;
        storage::save_space(space).map_err(|e| e.to_string())?;
    }
    Ok(())
}
