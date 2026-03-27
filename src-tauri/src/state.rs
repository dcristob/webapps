use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

use crate::config::models::*;

pub struct AppState {
    pub global_config: Mutex<GlobalConfig>,
    pub spaces: Mutex<Vec<SpaceConfig>>,
    pub active_space_id: Mutex<String>,
    pub active_app_id: Mutex<Option<String>>,
    pub webview_labels: Mutex<HashMap<String, String>>,
    /// Tracks (space_id, app_id) for the most recent app context-menu right-click.
    pub context_menu_target: Mutex<Option<(String, String)>>,
    /// Tracks space_id for the most recent space context-menu right-click.
    pub space_context_menu_target: Mutex<Option<String>>,
    /// Last time each app was actively viewed (app_id -> Instant).
    pub last_active: Mutex<HashMap<String, Instant>>,
    /// Apps whose webviews were destroyed to save memory but are still "open" in the sidebar.
    pub slept_apps: Mutex<HashSet<String>>,
}
