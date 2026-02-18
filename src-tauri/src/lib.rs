mod commands;
mod config;
mod state;

use config::models::*;
use config::storage;
use state::AppState;
use std::collections::HashMap;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    storage::ensure_dirs().expect("Failed to create config directories");

    let mut spaces = storage::list_spaces().unwrap_or_default();
    if spaces.is_empty() {
        let general = SpaceConfig {
            space: SpaceInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: "folder".to_string(),
                isolation: IsolationMode::default(),
            },
            apps: vec![],
        };
        storage::save_space(&general).expect("Failed to save default space");
        spaces.push(general);
    }

    let global_config = storage::load_global_config().unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            global_config: Mutex::new(global_config),
            spaces: Mutex::new(spaces),
            active_space_id: Mutex::new("general".to_string()),
            active_app_id: Mutex::new(None),
            webview_labels: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::spaces::list_spaces,
            commands::spaces::get_active_space,
            commands::spaces::create_space,
            commands::spaces::rename_space,
            commands::spaces::delete_space,
            commands::spaces::switch_space,
            commands::spaces::set_space_isolation,
            commands::apps::add_app,
            commands::apps::remove_app,
            commands::apps::edit_app,
            commands::apps::reorder_apps,
            commands::apps::get_apps_for_space,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
