use std::collections::HashMap;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const DIALOG_WIDTH: f64 = 450.0;
const DIALOG_HEIGHT: f64 = 400.0;

#[tauri::command]
pub fn show_dialog(app_handle: AppHandle, dialog_type: String, space_id: Option<String>, params: Option<HashMap<String, String>>) -> Result<(), String> {
    if let Some(existing) = app_handle.get_webview_window("dialog") {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut url = format!("index.html?dialog={}", dialog_type);
    if let Some(sid) = space_id {
        url.push_str(&format!("&spaceId={}", sid));
    }
    if let Some(extra) = params {
        for (key, value) in extra {
            url.push_str(&format!("&{}={}", key, urlencoding::encode(&value)));
        }
    }

    let title = match dialog_type.as_str() {
        "add-app" => "Add App",
        "edit-app" => "Edit App",
        "create-space" => "New Space",
        _ => "Dialog",
    };

    let main_window = app_handle.get_window("main").ok_or("Main window not found")?;
    let win_pos = main_window.outer_position().map_err(|e| e.to_string())?;
    let win_size = main_window.outer_size().map_err(|e| e.to_string())?;
    let scale = main_window.scale_factor().map_err(|e| e.to_string())?;

    let win_logical_w = win_size.width as f64 / scale;
    let win_logical_h = win_size.height as f64 / scale;
    let win_logical_x = win_pos.x as f64 / scale;
    let win_logical_y = win_pos.y as f64 / scale;

    let dialog_x = win_logical_x + (win_logical_w - DIALOG_WIDTH) / 2.0;
    let dialog_y = win_logical_y + (win_logical_h - DIALOG_HEIGHT) / 2.0;

    let dialog = WebviewWindowBuilder::new(
        &app_handle,
        "dialog",
        WebviewUrl::App(url.into()),
    )
    .title(title)
    .inner_size(DIALOG_WIDTH, DIALOG_HEIGHT)
    .position(dialog_x, dialog_y)
    .resizable(false)
    .decorations(false)
    .build()
    .map_err(|e| e.to_string())?;

    // Make dialog transient for the main window (on top of our app only, not all apps)
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        use gtk::prelude::*;
        let parent_gtk = main_window.gtk_window().map_err(|e| e.to_string())?;
        let dialog_gtk = dialog.as_ref().window().gtk_window().map_err(|e| e.to_string())?;
        dialog_gtk.set_transient_for(Some(&parent_gtk));
        dialog_gtk.set_modal(true);
    }

    Ok(())
}

#[tauri::command]
pub fn close_dialog(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("dialog") {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
