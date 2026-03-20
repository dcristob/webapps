mod commands;
mod config;
mod state;

use config::models::*;
use config::storage;
use state::AppState;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager};

const TOPBAR_HEIGHT: f64 = 48.0;

/// On COSMIC desktop, detect the dark/light mode preference and apply the
/// matching GTK theme so window decorations respect the system appearance.
#[cfg(target_os = "linux")]
fn apply_cosmic_theme() {
    use gtk::prelude::*;

    if std::env::var("XDG_CURRENT_DESKTOP").as_deref() != Ok("COSMIC") {
        return;
    }

    let is_dark = dirs::config_dir()
        .map(|c| c.join("cosmic/com.system76.CosmicTheme.Mode/v1/is_dark"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim() == "true")
        .unwrap_or(false);

    if let Some(settings) = gtk::Settings::default() {
        if is_dark {
            settings.set_property("gtk-theme-name", "adw-gtk3-dark");
            settings.set_property("gtk-application-prefer-dark-theme", true);
        } else {
            settings.set_property("gtk-theme-name", "adw-gtk3");
            settings.set_property("gtk-application-prefer-dark-theme", false);
        }
    }
}

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
    let sidebar_width = global_config.general.sidebar_width;

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            global_config: Mutex::new(global_config),
            spaces: Mutex::new(spaces),
            active_space_id: Mutex::new("general".to_string()),
            active_app_id: Mutex::new(None),
            webview_labels: Mutex::new(HashMap::new()),
            context_menu_target: Mutex::new(None),
        })
        .setup(move |app| {
            #[cfg(target_os = "linux")]
            apply_cosmic_theme();

            // Create a bare window (no default webview)
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
            let window = tauri::window::WindowBuilder::new(app, "main")
                .title("WebApps")
                .inner_size(1200.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .icon(icon)?
                .build()?;

            let size = window.inner_size()?;
            let scale = window.scale_factor()?;
            let logical_width = size.width as f64 / scale;
            let logical_height = size.height as f64 / scale;

            // --- Add topbar webview (full width, fixed height) ---
            let topbar_url = tauri::WebviewUrl::App("index.html?mode=topbar".into());
            let topbar_builder = tauri::WebviewBuilder::new("topbar", topbar_url);
            window.add_child(
                topbar_builder,
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(logical_width, TOPBAR_HEIGHT),
            )?;

            // On Linux: configure GTK layout for topbar + inner hbox
            #[cfg(target_os = "linux")]
            {
                use gtk::prelude::*;
                let vbox = window.default_vbox()?;
                // vbox is vertical by default — keep it that way

                // Topbar: non-expanding, fixed height
                let children = vbox.children();
                if let Some(topbar_widget) = children.last() {
                    vbox.set_child_packing(topbar_widget, false, false, 0, gtk::PackType::Start);
                    topbar_widget.set_size_request(-1, TOPBAR_HEIGHT as i32);
                }

                // Create inner horizontal box for sidebar + app webviews
                let inner_hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                vbox.pack_start(&inner_hbox, true, true, 0);
                inner_hbox.show();
            }

            // --- Add sidebar webview ---
            let sidebar_url = tauri::WebviewUrl::App("index.html".into());
            let sidebar_builder = tauri::WebviewBuilder::new("sidebar", sidebar_url);
            window.add_child(
                sidebar_builder,
                LogicalPosition::new(0.0, TOPBAR_HEIGHT),
                LogicalSize::new(sidebar_width as f64, logical_height - TOPBAR_HEIGHT),
            )?;

            // On Linux: reparent sidebar from vbox into the inner hbox
            #[cfg(target_os = "linux")]
            {
                use gtk::prelude::*;
                let vbox = window.default_vbox()?;
                let children = vbox.children();
                // children: [topbar, inner_hbox, sidebar]
                let sidebar_widget = children.last().cloned();
                let inner_hbox_widget = children.get(1).cloned();

                if let (Some(sidebar_w), Some(inner_w)) = (sidebar_widget, inner_hbox_widget) {
                    if let Some(inner_hbox) = inner_w.downcast_ref::<gtk::Box>() {
                        vbox.remove(&sidebar_w);
                        inner_hbox.pack_start(&sidebar_w, false, false, 0);
                        sidebar_w.set_size_request(sidebar_width as i32, -1);
                    }
                }
            }

            Ok(())
        })
        .on_menu_event(|app_handle, event| {
            let state = app_handle.state::<AppState>();
            let target = {
                let mut guard = state.context_menu_target.lock().unwrap();
                guard.take()
            };

            if let Some((space_id, app_id)) = target {
                match event.id().as_ref() {
                    "ctx-remove-app" => {
                        let _ = app_handle.emit("context-menu-remove-app", serde_json::json!({
                            "space_id": space_id,
                            "app_id": app_id,
                        }));
                    }
                    "ctx-edit-app" => {
                        let spaces = state.spaces.lock().unwrap();
                        if let Some(space) = spaces.iter().find(|s| s.space.id == space_id) {
                            if let Some(app) = space.apps.iter().find(|a| a.id == app_id) {
                                let _ = app_handle.emit("context-menu-edit-app", serde_json::json!({
                                    "space_id": space_id,
                                    "app_id": app_id,
                                    "name": app.name,
                                    "url": app.url,
                                    "icon": app.icon,
                                }));
                            }
                        }
                    }
                    _ => {}
                }
            }
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
            commands::webviews::open_app,
            commands::webviews::switch_to_app,
            commands::webviews::close_app,
            commands::webviews::hide_all_app_webviews,
            commands::webviews::get_active_app,
            commands::webviews::show_app_context_menu,
            commands::webviews::webview_go_back,
            commands::webviews::webview_reload,
            commands::favicon::fetch_site_info,
            commands::dialog::show_dialog,
            commands::dialog::close_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
