use std::collections::HashMap;
use std::sync::Mutex;

use crate::config::models::*;

pub struct AppState {
    pub global_config: Mutex<GlobalConfig>,
    pub spaces: Mutex<Vec<SpaceConfig>>,
    pub active_space_id: Mutex<String>,
    pub active_app_id: Mutex<Option<String>>,
    pub webview_labels: Mutex<HashMap<String, String>>,
}
