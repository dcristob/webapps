use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const DIALOG_WIDTH: f64 = 450.0;
const DIALOG_HEIGHT: f64 = 300.0;

#[tauri::command]
pub fn show_dialog(app_handle: AppHandle, dialog_type: String, space_id: Option<String>) -> Result<(), String> {
    // If dialog already exists, focus it
    if let Some(existing) = app_handle.get_webview_window("dialog") {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut url = format!("index.html?dialog={}", dialog_type);
    if let Some(sid) = space_id {
        url.push_str(&format!("&spaceId={}", sid));
    }

    let title = match dialog_type.as_str() {
        "add-app" => "Add App",
        "create-space" => "New Space",
        _ => "Dialog",
    };

    // Center dialog on the main window
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

    WebviewWindowBuilder::new(
        &app_handle,
        "dialog",
        WebviewUrl::App(url.into()),
    )
    .title(title)
    .inner_size(DIALOG_WIDTH, DIALOG_HEIGHT)
    .position(dialog_x, dialog_y)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn close_dialog(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("dialog") {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
