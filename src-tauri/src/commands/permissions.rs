use tauri::{AppHandle, Emitter, State, Webview};

use crate::config::models::{AppPermissions, MediaKind, PermissionState};
use crate::config::storage;
use crate::state::AppState;

#[tauri::command]
pub fn check_app_media_permissions(
    webview: Webview,
    state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    let label = webview.label().to_string();
    let app_id = label
        .strip_prefix("app-")
        .ok_or_else(|| format!("Not an app webview: {}", label))?;
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    for space in spaces.iter() {
        if let Some(app) = space.apps.iter().find(|a| a.id == app_id) {
            return Ok(app.permissions.clone());
        }
    }
    Err(format!("App '{}' not found", app_id))
}

#[tauri::command]
pub fn set_app_permission(
    app_handle: AppHandle,
    space_id: String,
    app_id: String,
    kind: MediaKind,
    state_value: PermissionState,
    state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter_mut()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    let app = space
        .apps
        .iter_mut()
        .find(|a| a.id == app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;

    match kind {
        MediaKind::Camera => app.permissions.camera = state_value,
        MediaKind::Microphone => app.permissions.microphone = state_value,
    }

    let perms = app.permissions.clone();
    storage::save_space(space).map_err(|e| e.to_string())?;

    let _ = app_handle.emit(
        "media-permission-changed",
        serde_json::json!({
            "app_id": app_id,
            "permissions": perms,
        }),
    );

    Ok(perms)
}

#[tauri::command]
pub fn get_app_permissions(
    space_id: String,
    app_id: String,
    state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    let app = space
        .apps
        .iter()
        .find(|a| a.id == app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;
    Ok(app.permissions.clone())
}

#[tauri::command]
#[cfg(target_os = "linux")]
pub fn respond_media_permission(
    app_handle: AppHandle,
    space_id: String,
    app_id: String,
    camera: Option<PermissionState>,
    microphone: Option<PermissionState>,
    state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    use webkit2gtk::PermissionRequestExt;

    // 1. Persist the user's decisions.
    let perms = {
        let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
        let space = spaces
            .iter_mut()
            .find(|s| s.space.id == space_id)
            .ok_or_else(|| format!("Space '{}' not found", space_id))?;
        let app = space
            .apps
            .iter_mut()
            .find(|a| a.id == app_id)
            .ok_or_else(|| format!("App '{}' not found", app_id))?;
        if let Some(c) = camera {
            app.permissions.camera = c;
        }
        if let Some(m) = microphone {
            app.permissions.microphone = m;
        }
        let perms = app.permissions.clone();
        storage::save_space(space).map_err(|e| e.to_string())?;
        perms
    };

    // 2. Resolve the pending WebKit request, if any.
    let pending = {
        let mut pending_map = state.pending_media_requests.lock().map_err(|e| e.to_string())?;
        pending_map.remove(&app_id)
    };

    if let Some(p) = pending {
        let camera_ok = !p.wants_camera || perms.camera == PermissionState::Allow;
        let mic_ok = !p.wants_microphone || perms.microphone == PermissionState::Allow;
        if camera_ok && mic_ok {
            p.request.allow();
        } else {
            p.request.deny();
        }
    }

    let _ = app_handle.emit(
        "media-permission-changed",
        serde_json::json!({
            "app_id": app_id,
            "permissions": perms,
        }),
    );

    Ok(perms)
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
pub fn respond_media_permission(
    _app_handle: AppHandle,
    _space_id: String,
    _app_id: String,
    _camera: Option<PermissionState>,
    _microphone: Option<PermissionState>,
    _state: State<'_, AppState>,
) -> Result<AppPermissions, String> {
    Err("Media permissions are only supported on Linux for now".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::{AppConfig, IsolationMode, SpaceConfig, SpaceInfo};

    fn sample_space() -> SpaceConfig {
        SpaceConfig {
            space: SpaceInfo {
                id: "s1".to_string(),
                name: "S1".to_string(),
                icon: "folder".to_string(),
                color: "#000000".to_string(),
                isolation: IsolationMode::Shared,
            },
            apps: vec![AppConfig {
                id: "a1".to_string(),
                name: "A1".to_string(),
                url: "https://example.com".to_string(),
                icon: "auto".to_string(),
                isolation_override: false,
                permissions: AppPermissions::default(),
            }],
        }
    }

    #[test]
    fn mutating_camera_does_not_touch_microphone() {
        let mut space = sample_space();
        let app = space.apps.iter_mut().find(|a| a.id == "a1").unwrap();
        app.permissions.camera = PermissionState::Allow;
        assert_eq!(app.permissions.camera, PermissionState::Allow);
        assert_eq!(app.permissions.microphone, PermissionState::Ask);
    }
}
