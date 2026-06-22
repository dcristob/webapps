use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

use crate::config::models::*;

#[cfg(target_os = "linux")]
use webkit2gtk::UserMediaPermissionRequest;

pub struct AppState {
    pub global_config: Mutex<GlobalConfig>,
    pub spaces: Mutex<Vec<SpaceConfig>>,
    pub active_space_id: Mutex<String>,
    pub active_app_id: Mutex<Option<String>>,
    /// Whether the sidebar webview is currently shown (toggled by Ctrl+B).
    pub sidebar_visible: Mutex<bool>,
    pub webview_labels: Mutex<HashMap<String, String>>,
    /// Tracks (space_id, app_id) for the most recent app context-menu right-click.
    pub context_menu_target: Mutex<Option<(String, String)>>,
    /// Tracks space_id for the most recent space context-menu right-click.
    pub space_context_menu_target: Mutex<Option<String>>,
    /// Last time each app was actively viewed (app_id -> Instant).
    pub last_active: Mutex<HashMap<String, Instant>>,
    /// Apps whose webviews were destroyed to save memory but are still "open" in the sidebar.
    pub slept_apps: Mutex<HashSet<String>>,
    /// Map of app_id -> oneshot sender awaiting the favicon URL list captured
    /// from that app's live webview by `refetch_app_icon` / `capture_favicon_done`.
    pub pending_icon_captures: Mutex<HashMap<String, tokio::sync::oneshot::Sender<Vec<String>>>>,

    /// Map of app_id -> pending WebKit media permission request waiting for the user.
    /// `wants_camera` / `wants_microphone` flags say which devices the page asked for.
    #[cfg(target_os = "linux")]
    pub pending_media_requests: Mutex<HashMap<String, PendingMediaRequest>>,

    /// Map of app_id -> currently-capturing flags, updated by WebKit capture-state signals.
    pub active_captures: Mutex<HashMap<String, ActiveCaptures>>,
}

#[cfg(target_os = "linux")]
pub struct PendingMediaRequest {
    pub request: UserMediaPermissionRequest,
    pub wants_camera: bool,
    pub wants_microphone: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ActiveCaptures {
    pub camera: bool,
    pub microphone: bool,
}

// SAFETY: WebKit objects are not Send by default, but we only ever touch the
// PermissionRequest from the GTK main thread (where the signal is emitted and
// where webview.eval / state operations run via Tauri's main-thread tasks).
// Mutex requires Send; Tauri State requires Sync. We mark PendingMediaRequest
// Send + Sync because access is serialized to the GTK main thread by virtue
// of being called only from Tauri commands and signal handlers.
#[cfg(target_os = "linux")]
unsafe impl Send for PendingMediaRequest {}
#[cfg(target_os = "linux")]
unsafe impl Sync for PendingMediaRequest {}
