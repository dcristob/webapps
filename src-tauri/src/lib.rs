mod commands;
mod config;
mod render_mode;
mod state;

use config::models::*;
use config::storage;
use state::AppState;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
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

    // Configure the WebKitGTK DMA-BUF renderer before any GTK/WebKit init.
    // Self-heals on systems where the GPU renderer crashes at startup.
    #[cfg(target_os = "linux")]
    render_mode::configure_for_launch();

    let mut spaces = storage::list_spaces().unwrap_or_default();
    if spaces.is_empty() {
        let general = SpaceConfig {
            space: SpaceInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: "folder".to_string(),
                color: "#4a9eff".to_string(),
                isolation: IsolationMode::default(),
            },
            apps: vec![],
        };
        storage::save_space(&general).expect("Failed to save default space");
        spaces.push(general);
    }

    let global_config = storage::load_global_config().unwrap_or_default();
    let sidebar_width = global_config.general.sidebar_width;
    let sidebar_visible = global_config.general.sidebar_visible;

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            global_config: Mutex::new(global_config),
            spaces: Mutex::new(spaces),
            active_space_id: Mutex::new("general".to_string()),
            active_app_id: Mutex::new(None),
            sidebar_visible: Mutex::new(sidebar_visible),
            webview_labels: Mutex::new(HashMap::new()),
            context_menu_target: Mutex::new(None),
            space_context_menu_target: Mutex::new(None),
            last_active: Mutex::new(HashMap::new()),
            slept_apps: Mutex::new(std::collections::HashSet::new()),
            pending_icon_captures: Mutex::new(HashMap::new()),
            #[cfg(target_os = "linux")]
            pending_media_requests: Mutex::new(HashMap::new()),
            active_captures: Mutex::new(HashMap::new()),
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

            // On Linux: draw a 1px white border around the window while it is
            // focused. Budgie/Mutter on Wayland never draws server-side
            // decorations, and our webviews (the content one loads third-party
            // sites) cover the window edges, so the GTK theme's focused-window
            // border can't show through. Instead we pad the window by 1px and
            // paint its own background white; GTK's `:backdrop` state removes it
            // when the window loses focus.
            #[cfg(target_os = "linux")]
            {
                use gtk::prelude::*;
                if let Ok(gtk_window) = window.gtk_window() {
                    let provider = gtk::CssProvider::new();
                    let _ = provider.load_from_data(
                        b"window { background-color: #ffffff; padding: 1px; } \
                          window:backdrop { background-color: transparent; }",
                    );
                    gtk_window.style_context().add_provider(
                        &provider,
                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                    );
                }
            }

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

            // Background task: periodically sleep idle app webviews to free memory
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(60));

                    let state = app_handle.state::<AppState>();

                    let timeout_mins = {
                        let config = state.global_config.lock().unwrap();
                        config.general.sleep_timeout_mins
                    };
                    if timeout_mins == 0 {
                        continue;
                    }
                    let timeout = Duration::from_secs(timeout_mins as u64 * 60);

                    let active_app = {
                        state.active_app_id.lock().unwrap().clone()
                    };

                    let candidates: Vec<String> = {
                        let last_active = state.last_active.lock().unwrap();
                        let labels = state.webview_labels.lock().unwrap();
                        let now = Instant::now();
                        labels.keys()
                            .filter(|app_id| {
                                // Never sleep the currently active app
                                if active_app.as_deref() == Some(app_id.as_str()) {
                                    return false;
                                }
                                match last_active.get(*app_id) {
                                    Some(t) => now.duration_since(*t) >= timeout,
                                    None => false,
                                }
                            })
                            .cloned()
                            .collect()
                    };

                    for app_id in candidates {
                        let _ = commands::webviews::sleep_app_inner(&app_handle, &app_id, &state);
                    }
                }
            });

            // Confirm startup survived so future launches know the DMA-BUF
            // renderer is safe on this system. A startup GPU crash kills the
            // process well within this delay, so a stale "probing" marker is
            // left for the next launch to self-heal from.
            #[cfg(target_os = "linux")]
            {
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_secs(2));
                    render_mode::confirm_started_ok();
                });
            }

            Ok(())
        })
        .on_menu_event(|app_handle, event| {
            let state = app_handle.state::<AppState>();

            // Handle app context menu events
            let app_target = {
                let mut guard = state.context_menu_target.lock().unwrap();
                guard.take()
            };

            if let Some((space_id, app_id)) = app_target {
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

            // Handle space context menu events
            let space_target = {
                let mut guard = state.space_context_menu_target.lock().unwrap();
                guard.take()
            };

            if let Some(space_id) = space_target {
                match event.id().as_ref() {
                    "ctx-edit-space" => {
                        let spaces = state.spaces.lock().unwrap();
                        if let Some(space) = spaces.iter().find(|s| s.space.id == space_id) {
                            let _ = app_handle.emit("context-menu-edit-space", serde_json::json!({
                                "space_id": space_id,
                                "name": space.space.name,
                                "color": space.space.color,
                            }));
                        }
                    }
                    "ctx-delete-space" => {
                        let _ = app_handle.emit("context-menu-delete-space", serde_json::json!({
                            "space_id": space_id,
                        }));
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
            commands::spaces::edit_space,
            commands::spaces::reorder_spaces,
            commands::spaces::set_space_isolation,
            commands::spaces::show_space_context_menu,
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
            commands::webviews::toggle_sidebar,
            commands::webviews::webview_go_back,
            commands::webviews::webview_reload,
            commands::webviews::get_slept_apps,
            commands::webviews::open_in_browser,
            commands::webviews::open_blank_popup,
            commands::webviews::eval_in_app,
            commands::favicon::fetch_site_info,
            commands::favicon::capture_favicon_done,
            commands::favicon::refetch_app_icon,
            commands::dialog::show_dialog,
            commands::dialog::close_dialog,
            commands::dialog::open_space_switcher,
            commands::dialog::focus_active_app,
            commands::permissions::set_app_permission,
            commands::permissions::get_app_permissions,
            commands::permissions::respond_media_permission,
            commands::permissions::check_app_media_permissions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
