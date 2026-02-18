# Phase 1: Core App (MVP) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a working Tauri v2 desktop app with a sidebar, Spaces, app management, webview lifecycle, notification badges, and TOML config persistence.

**Architecture:** Single OS window with stacked webviews. The sidebar is a Svelte+TS webview pinned to the left. Each user-added app is a separate Tauri webview whose visibility is toggled. The Rust backend manages webview lifecycle, config persistence, and IPC commands.

**Tech Stack:** Tauri v2 (with `unstable` feature), Svelte 5 + TypeScript, Rust, TOML (via `toml` crate), `reqwest` (favicon fetching), Vite.

**Reference docs:**
- Design: `docs/plans/2026-02-18-webapps-design.md`
- Requirements: `docs/REQUIREMENTS.md`
- Conventions: `CLAUDE.md`

---

### Task 1: Scaffold Tauri v2 + Svelte + TypeScript Project

**Files:**
- Create: entire project scaffold via `create-tauri-app`
- Modify: `src-tauri/Cargo.toml` (add `unstable` feature)
- Modify: `src-tauri/tauri.conf.json` (window config)

**Step 1: Create the project**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps
npm create tauri-app@latest . -- --template svelte-ts
```

If prompted, select:
- Package manager: npm
- UI template: Svelte
- UI flavor: TypeScript

Expected: Project scaffold created with `src/`, `src-tauri/`, `package.json`, `vite.config.ts`, etc.

**Step 2: Enable the `unstable` feature for multi-webview**

In `src-tauri/Cargo.toml`, add the `unstable` feature to the `tauri` dependency:

```toml
[dependencies]
tauri = { version = "2", features = ["unstable"] }
```

**Step 3: Configure the main window**

In `src-tauri/tauri.conf.json`, update the window config to set up our single main window:

```json
{
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "WebApps",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "decorations": true
      }
    ]
  }
}
```

**Step 4: Install dependencies and verify it builds**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps
npm install
npm run tauri dev
```

Expected: A Tauri window opens showing the default Svelte template. Close it.

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: scaffold Tauri v2 + Svelte + TypeScript project"
```

---

### Task 2: Define Rust Data Models and Config Module

**Files:**
- Create: `src-tauri/src/config/mod.rs`
- Create: `src-tauri/src/config/models.rs`
- Create: `src-tauri/src/config/storage.rs`
- Modify: `src-tauri/src/main.rs` (add `mod config`)
- Modify: `src-tauri/Cargo.toml` (add dependencies)

**Step 1: Add Rust dependencies**

In `src-tauri/Cargo.toml`, add:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
dirs = "6"
uuid = { version = "1", features = ["v4"] }
thiserror = "2"
reqwest = { version = "0.12", features = ["blocking"] }
```

**Step 2: Create the data models**

Create `src-tauri/src/config/models.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub general: GeneralSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    pub sidebar_width: u32,
    pub theme: String,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            general: GeneralSettings {
                sidebar_width: 250,
                theme: "dark".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceConfig {
    pub space: SpaceInfo,
    pub apps: Vec<AppConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceInfo {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub isolation: IsolationMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum IsolationMode {
    Shared,
    PerApp,
}

impl Default for IsolationMode {
    fn default() -> Self {
        Self::Shared
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub icon: String,
    pub isolation_override: bool,
}
```

**Step 3: Create the storage module**

Create `src-tauri/src/config/storage.rs`:

```rust
use std::fs;
use std::path::PathBuf;

use crate::config::models::*;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("Config directory not found")]
    NoCOnfigDir,
}

pub fn config_dir() -> Result<PathBuf, ConfigError> {
    let base = dirs::config_dir().ok_or(ConfigError::NoCOnfigDir)?;
    Ok(base.join("webapps"))
}

pub fn ensure_dirs() -> Result<(), ConfigError> {
    let dir = config_dir()?;
    fs::create_dir_all(dir.join("spaces"))?;
    fs::create_dir_all(dir.join("webview-data"))?;
    Ok(())
}

pub fn load_global_config() -> Result<GlobalConfig, ConfigError> {
    let path = config_dir()?.join("config.toml");
    if !path.exists() {
        let config = GlobalConfig::default();
        save_global_config(&config)?;
        return Ok(config);
    }
    let content = fs::read_to_string(&path)?;
    Ok(toml::from_str(&content)?)
}

pub fn save_global_config(config: &GlobalConfig) -> Result<(), ConfigError> {
    let path = config_dir()?.join("config.toml");
    let content = toml::to_string_pretty(config)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn load_space(space_id: &str) -> Result<SpaceConfig, ConfigError> {
    let path = config_dir()?.join("spaces").join(format!("{}.toml", space_id));
    let content = fs::read_to_string(&path)?;
    Ok(toml::from_str(&content)?)
}

pub fn save_space(space: &SpaceConfig) -> Result<(), ConfigError> {
    let path = config_dir()?.join("spaces").join(format!("{}.toml", space.space.id));
    let content = toml::to_string_pretty(space)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn list_spaces() -> Result<Vec<SpaceConfig>, ConfigError> {
    let spaces_dir = config_dir()?.join("spaces");
    if !spaces_dir.exists() {
        return Ok(vec![]);
    }
    let mut spaces = Vec::new();
    for entry in fs::read_dir(&spaces_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "toml") {
            let content = fs::read_to_string(&path)?;
            spaces.push(toml::from_str(&content)?);
        }
    }
    Ok(spaces)
}

pub fn delete_space_file(space_id: &str) -> Result<(), ConfigError> {
    let path = config_dir()?.join("spaces").join(format!("{}.toml", space_id));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn webview_data_dir(space_id: &str, app_id: Option<&str>) -> Result<PathBuf, ConfigError> {
    let base = config_dir()?.join("webview-data").join(format!("space-{}", space_id));
    match app_id {
        Some(id) => Ok(base.join(id)),
        None => Ok(base),
    }
}
```

**Step 4: Create the config module**

Create `src-tauri/src/config/mod.rs`:

```rust
pub mod models;
pub mod storage;
```

**Step 5: Wire up in main.rs**

Add `mod config;` to `src-tauri/src/main.rs` (keep existing content, just add the module declaration).

**Step 6: Verify it compiles**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps/src-tauri
cargo check
```

Expected: Compiles without errors.

**Step 7: Commit**

```bash
git add src-tauri/src/config/ src-tauri/Cargo.toml src-tauri/src/main.rs
git commit -m "feat: add config data models and TOML storage module"
```

---

### Task 3: Build the App State and IPC Commands for Spaces

**Files:**
- Create: `src-tauri/src/state.rs`
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/spaces.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create the shared app state**

Create `src-tauri/src/state.rs`:

```rust
use std::collections::HashMap;
use std::sync::Mutex;

use crate::config::models::*;

pub struct AppState {
    pub global_config: Mutex<GlobalConfig>,
    pub spaces: Mutex<Vec<SpaceConfig>>,
    pub active_space_id: Mutex<String>,
    pub active_app_id: Mutex<Option<String>>,
    /// Maps app_id -> webview label for tracking created webviews
    pub webview_labels: Mutex<HashMap<String, String>>,
}
```

**Step 2: Create space IPC commands**

Create `src-tauri/src/commands/spaces.rs`:

```rust
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
pub fn rename_space(
    space_id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
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

    // If deleted space was active, switch to general
    let mut active = state.active_space_id.lock().map_err(|e| e.to_string())?;
    if *active == space_id {
        *active = "general".to_string();
    }

    Ok(())
}

#[tauri::command]
pub fn switch_space(space_id: String, state: State<'_, AppState>) -> Result<SpaceConfig, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter()
        .find(|s| s.space.id == space_id)
        .cloned()
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;

    let mut active = state.active_space_id.lock().map_err(|e| e.to_string())?;
    *active = space_id;

    // Clear active app when switching spaces
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = None;

    Ok(space)
}

#[tauri::command]
pub fn set_space_isolation(
    space_id: String,
    mode: IsolationMode,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    if let Some(space) = spaces.iter_mut().find(|s| s.space.id == space_id) {
        space.space.isolation = mode;
        storage::save_space(space).map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

**Step 3: Create commands module**

Create `src-tauri/src/commands/mod.rs`:

```rust
pub mod spaces;
```

**Step 4: Wire up state and commands in main.rs**

Replace the contents of `src-tauri/src/main.rs` with:

```rust
mod commands;
mod config;
mod state;

use config::models::*;
use config::storage;
use state::AppState;
use std::collections::HashMap;
use std::sync::Mutex;

fn main() {
    // Ensure config directories exist
    storage::ensure_dirs().expect("Failed to create config directories");

    // Load or create default spaces
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 5: Verify it compiles**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps/src-tauri
cargo check
```

Expected: Compiles without errors.

**Step 6: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/commands/ src-tauri/src/main.rs
git commit -m "feat: add app state and Space management IPC commands"
```

---

### Task 4: Build IPC Commands for App Management

**Files:**
- Create: `src-tauri/src/commands/apps.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create app management commands**

Create `src-tauri/src/commands/apps.rs`:

```rust
use tauri::State;
use uuid::Uuid;

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;

#[tauri::command]
pub fn add_app(
    space_id: String,
    name: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let app = AppConfig {
        id: Uuid::new_v4().to_string(),
        name,
        url,
        icon: "auto".to_string(),
        isolation_override: false,
    };

    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter_mut()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    space.apps.push(app.clone());
    storage::save_space(space).map_err(|e| e.to_string())?;

    Ok(app)
}

#[tauri::command]
pub fn remove_app(
    space_id: String,
    app_id: String,
    delete_data: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter_mut()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    space.apps.retain(|a| a.id != app_id);
    storage::save_space(space).map_err(|e| e.to_string())?;

    if delete_data {
        let data_dir = storage::webview_data_dir(&space_id, Some(&app_id))
            .map_err(|e| e.to_string())?;
        if data_dir.exists() {
            std::fs::remove_dir_all(&data_dir).map_err(|e| e.to_string())?;
        }
    }

    // Remove from active app if it was active
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    if active_app.as_deref() == Some(&app_id) {
        *active_app = None;
    }

    Ok(())
}

#[tauri::command]
pub fn edit_app(
    space_id: String,
    app_id: String,
    name: Option<String>,
    url: Option<String>,
    icon: Option<String>,
    isolation_override: Option<bool>,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
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

    if let Some(n) = name {
        app.name = n;
    }
    if let Some(u) = url {
        app.url = u;
    }
    if let Some(i) = icon {
        app.icon = i;
    }
    if let Some(iso) = isolation_override {
        app.isolation_override = iso;
    }

    storage::save_space(space).map_err(|e| e.to_string())?;
    Ok(app.clone())
}

#[tauri::command]
pub fn reorder_apps(
    space_id: String,
    app_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter_mut()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;

    let mut reordered = Vec::new();
    for id in &app_ids {
        if let Some(app) = space.apps.iter().find(|a| &a.id == id) {
            reordered.push(app.clone());
        }
    }
    space.apps = reordered;
    storage::save_space(space).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_apps_for_space(
    space_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AppConfig>, String> {
    let spaces = state.spaces.lock().map_err(|e| e.to_string())?;
    let space = spaces
        .iter()
        .find(|s| s.space.id == space_id)
        .ok_or_else(|| format!("Space '{}' not found", space_id))?;
    Ok(space.apps.clone())
}
```

**Step 2: Update commands module**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod apps;
pub mod spaces;
```

**Step 3: Register commands in main.rs**

Add the new commands to the `invoke_handler` in `src-tauri/src/main.rs`:

```rust
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
```

**Step 4: Verify it compiles**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps/src-tauri
cargo check
```

Expected: Compiles without errors.

**Step 5: Commit**

```bash
git add src-tauri/src/commands/ src-tauri/src/main.rs
git commit -m "feat: add app management IPC commands (add, remove, edit, reorder)"
```

---

### Task 5: Build Webview Lifecycle Commands

**Files:**
- Create: `src-tauri/src/commands/webviews.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create webview management commands**

Create `src-tauri/src/commands/webviews.rs`:

```rust
use std::fs;

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, State, WebviewBuilder, WebviewUrl};

use crate::config::models::*;
use crate::config::storage;
use crate::state::AppState;

fn resolve_data_directory(
    space: &SpaceConfig,
    app: &AppConfig,
) -> Result<std::path::PathBuf, String> {
    let use_per_app =
        space.space.isolation == IsolationMode::PerApp || app.isolation_override;

    if use_per_app {
        storage::webview_data_dir(&space.space.id, Some(&app.id))
    } else {
        storage::webview_data_dir(&space.space.id, None)
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_app(
    app_handle: AppHandle,
    space_id: String,
    app_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
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

    let label = format!("app-{}", app.id);

    // Check if webview already exists
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    if labels.contains_key(&app.id) {
        // Webview exists, just show it
        drop(labels);
        return switch_to_app(app_handle, space_id, app_id, state);
    }
    drop(labels);

    let data_dir = resolve_data_directory(space, app)?;
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    let window = app_handle
        .get_window("main")
        .ok_or("Main window not found")?;

    let sidebar_width = {
        let config = state.global_config.lock().map_err(|e| e.to_string())?;
        config.general.sidebar_width
    };

    let window_size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let logical_width = (window_size.width as f64 / scale) as f64;
    let logical_height = (window_size.height as f64 / scale) as f64;

    let webview_url = WebviewUrl::External(app.url.parse().map_err(|e: url::ParseError| e.to_string())?);

    let webview_builder = WebviewBuilder::new(&label, webview_url)
        .auto_resize()
        .data_directory(data_dir);

    window
        .add_child(
            webview_builder,
            LogicalPosition::new(sidebar_width as f64, 0.0),
            LogicalSize::new(logical_width - sidebar_width as f64, logical_height),
        )
        .map_err(|e| e.to_string())?;

    // Track the webview
    let mut labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    labels.insert(app.id.clone(), label);

    // Set as active app
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = Some(app.id.clone());

    Ok(())
}

#[tauri::command]
pub fn switch_to_app(
    app_handle: AppHandle,
    _space_id: String,
    app_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;

    // Hide all webviews
    for (_, label) in labels.iter() {
        if let Some(webview) = app_handle.get_webview(label) {
            webview.set_size(LogicalSize::new(0.0, 0.0)).map_err(|e| e.to_string())?;
        }
    }

    // Show the target webview
    if let Some(label) = labels.get(&app_id) {
        if let Some(webview) = app_handle.get_webview(label) {
            let window = app_handle
                .get_window("main")
                .ok_or("Main window not found")?;
            let sidebar_width = {
                let config = state.global_config.lock().map_err(|e| e.to_string())?;
                config.general.sidebar_width
            };
            let window_size = window.inner_size().map_err(|e| e.to_string())?;
            let scale = window.scale_factor().map_err(|e| e.to_string())?;
            let logical_width = (window_size.width as f64 / scale) as f64;
            let logical_height = (window_size.height as f64 / scale) as f64;

            webview
                .set_position(LogicalPosition::new(sidebar_width as f64, 0.0))
                .map_err(|e| e.to_string())?;
            webview
                .set_size(LogicalSize::new(
                    logical_width - sidebar_width as f64,
                    logical_height,
                ))
                .map_err(|e| e.to_string())?;
        }
    }

    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = Some(app_id);

    Ok(())
}

#[tauri::command]
pub fn close_app(
    app_handle: AppHandle,
    app_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut labels = state.webview_labels.lock().map_err(|e| e.to_string())?;

    if let Some(label) = labels.remove(&app_id) {
        if let Some(webview) = app_handle.get_webview(&label) {
            webview.close().map_err(|e| e.to_string())?;
        }
    }

    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    if active_app.as_deref() == Some(&app_id) {
        *active_app = None;
    }

    Ok(())
}

#[tauri::command]
pub fn hide_all_app_webviews(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let labels = state.webview_labels.lock().map_err(|e| e.to_string())?;
    for (_, label) in labels.iter() {
        if let Some(webview) = app_handle.get_webview(label) {
            webview.set_size(LogicalSize::new(0.0, 0.0)).map_err(|e| e.to_string())?;
        }
    }
    let mut active_app = state.active_app_id.lock().map_err(|e| e.to_string())?;
    *active_app = None;
    Ok(())
}

#[tauri::command]
pub fn get_active_app(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let active = state.active_app_id.lock().map_err(|e| e.to_string())?;
    Ok(active.clone())
}
```

**Step 2: Update commands module**

In `src-tauri/src/commands/mod.rs`:

```rust
pub mod apps;
pub mod spaces;
pub mod webviews;
```

**Step 3: Register in main.rs**

Add webview commands to the invoke handler:

```rust
commands::webviews::open_app,
commands::webviews::switch_to_app,
commands::webviews::close_app,
commands::webviews::hide_all_app_webviews,
commands::webviews::get_active_app,
```

Also add `url` dependency in `Cargo.toml`:

```toml
url = "2"
```

**Step 4: Verify it compiles**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps/src-tauri
cargo check
```

Expected: Compiles without errors. There may be warnings about unused imports — those are fine during development.

**Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat: add webview lifecycle commands (open, switch, close, hide)"
```

---

### Task 6: Build Favicon Fetching Command

**Files:**
- Create: `src-tauri/src/commands/favicon.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create the favicon command**

Create `src-tauri/src/commands/favicon.rs`:

```rust
use std::fs;
use std::path::PathBuf;

use crate::config::storage;

/// Fetches favicon and page title from a URL.
/// Returns (title, icon_path) where icon_path is the saved favicon on disk.
#[tauri::command]
pub async fn fetch_site_info(url: String) -> Result<(String, String), String> {
    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let body = response.text().await.map_err(|e| e.to_string())?;

    // Extract title from HTML
    let title = extract_title(&body).unwrap_or_else(|| {
        // Fallback: use domain name
        url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| "Untitled".to_string())
    });

    // Extract favicon URL
    let favicon_url = extract_favicon_url(&body, &url)
        .unwrap_or_else(|| {
            // Fallback: try /favicon.ico
            if let Ok(parsed) = url::Url::parse(&url) {
                format!("{}://{}/favicon.ico", parsed.scheme(), parsed.host_str().unwrap_or(""))
            } else {
                String::new()
            }
        });

    // Download and save favicon
    let icon_path = if !favicon_url.is_empty() {
        download_favicon(&favicon_url, &title).await.unwrap_or_else(|_| "auto".to_string())
    } else {
        "auto".to_string()
    };

    Ok((title, icon_path))
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let tag_end = lower[start..].find('>')?;
    let content_start = start + tag_end + 1;
    let end = lower[content_start..].find("</title>")?;
    let title = &html[content_start..content_start + end];
    Some(title.trim().to_string())
}

fn extract_favicon_url(html: &str, page_url: &str) -> Option<String> {
    let lower = html.to_lowercase();

    // Look for <link rel="icon" or <link rel="shortcut icon"
    for rel in &["icon", "shortcut icon", "apple-touch-icon"] {
        if let Some(pos) = lower.find(&format!("rel=\"{}\"", rel)) {
            // Search around this position for href
            let search_start = if pos > 200 { pos - 200 } else { 0 };
            let search_end = std::cmp::min(pos + 200, html.len());
            let snippet = &html[search_start..search_end];
            let snippet_lower = snippet.to_lowercase();

            if let Some(href_pos) = snippet_lower.find("href=\"") {
                let href_start = href_pos + 6;
                if let Some(href_end) = snippet[href_start..].find('"') {
                    let href = &snippet[href_start..href_start + href_end];
                    return Some(resolve_url(href, page_url));
                }
            }
        }
    }
    None
}

fn resolve_url(href: &str, base: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if href.starts_with("//") {
        return format!("https:{}", href);
    }
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(resolved) = base_url.join(href) {
            return resolved.to_string();
        }
    }
    href.to_string()
}

async fn download_favicon(favicon_url: &str, title: &str) -> Result<String, String> {
    let response = reqwest::get(favicon_url).await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err("Failed to download favicon".to_string());
    }

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

    let icons_dir = storage::config_dir()
        .map_err(|e| e.to_string())?
        .join("icons");
    fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;

    // Determine extension from URL or content type
    let ext = if favicon_url.contains(".png") {
        "png"
    } else if favicon_url.contains(".svg") {
        "svg"
    } else {
        "ico"
    };

    let safe_name: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let filename = format!("{}_{}.{}", safe_name, &uuid::Uuid::new_v4().to_string()[..8], ext);
    let path = icons_dir.join(&filename);
    fs::write(&path, &bytes).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}
```

**Step 2: Update commands module and main.rs**

Add `pub mod favicon;` to `src-tauri/src/commands/mod.rs`.

Add `commands::favicon::fetch_site_info` to the invoke handler in `main.rs`.

**Step 3: Verify it compiles**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps/src-tauri
cargo check
```

**Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: add favicon and page title fetching command"
```

---

### Task 7: Set Up TypeScript Types and Tauri API Bindings

**Files:**
- Create: `src/lib/types/index.ts`
- Create: `src/lib/api.ts`

**Step 1: Define TypeScript types**

Create `src/lib/types/index.ts`:

```typescript
export interface SpaceInfo {
  id: string;
  name: string;
  icon: string;
  isolation: "shared" | "per-app";
}

export interface AppConfig {
  id: string;
  name: string;
  url: string;
  icon: string;
  isolation_override: boolean;
}

export interface SpaceConfig {
  space: SpaceInfo;
  apps: AppConfig[];
}

export interface GeneralSettings {
  sidebar_width: number;
  theme: string;
}

export interface GlobalConfig {
  general: GeneralSettings;
}
```

**Step 2: Create Tauri API wrapper**

Create `src/lib/api.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import type { SpaceConfig, AppConfig } from "./types";

// Space commands
export async function listSpaces(): Promise<SpaceConfig[]> {
  return invoke("list_spaces");
}

export async function getActiveSpace(): Promise<string> {
  return invoke("get_active_space");
}

export async function createSpace(name: string): Promise<SpaceConfig> {
  return invoke("create_space", { name });
}

export async function renameSpace(spaceId: string, newName: string): Promise<void> {
  return invoke("rename_space", { spaceId, newName });
}

export async function deleteSpace(spaceId: string): Promise<void> {
  return invoke("delete_space", { spaceId });
}

export async function switchSpace(spaceId: string): Promise<SpaceConfig> {
  return invoke("switch_space", { spaceId });
}

export async function setSpaceIsolation(
  spaceId: string,
  mode: "shared" | "per-app"
): Promise<void> {
  return invoke("set_space_isolation", { spaceId, mode });
}

// App commands
export async function addApp(
  spaceId: string,
  name: string,
  url: string
): Promise<AppConfig> {
  return invoke("add_app", { spaceId, name, url });
}

export async function removeApp(
  spaceId: string,
  appId: string,
  deleteData: boolean
): Promise<void> {
  return invoke("remove_app", { spaceId, appId, deleteData });
}

export async function editApp(
  spaceId: string,
  appId: string,
  updates: {
    name?: string;
    url?: string;
    icon?: string;
    isolationOverride?: boolean;
  }
): Promise<AppConfig> {
  return invoke("edit_app", {
    spaceId,
    appId,
    name: updates.name ?? null,
    url: updates.url ?? null,
    icon: updates.icon ?? null,
    isolationOverride: updates.isolationOverride ?? null,
  });
}

export async function reorderApps(
  spaceId: string,
  appIds: string[]
): Promise<void> {
  return invoke("reorder_apps", { spaceId, appIds });
}

export async function getAppsForSpace(
  spaceId: string
): Promise<AppConfig[]> {
  return invoke("get_apps_for_space", { spaceId });
}

// Webview commands
export async function openApp(spaceId: string, appId: string): Promise<void> {
  return invoke("open_app", { spaceId, appId });
}

export async function switchToApp(
  spaceId: string,
  appId: string
): Promise<void> {
  return invoke("switch_to_app", { spaceId, appId });
}

export async function closeApp(appId: string): Promise<void> {
  return invoke("close_app", { appId });
}

export async function hideAllAppWebviews(): Promise<void> {
  return invoke("hide_all_app_webviews");
}

export async function getActiveApp(): Promise<string | null> {
  return invoke("get_active_app");
}

// Favicon
export async function fetchSiteInfo(
  url: string
): Promise<[string, string]> {
  return invoke("fetch_site_info", { url });
}
```

**Step 3: Commit**

```bash
git add src/lib/
git commit -m "feat: add TypeScript types and Tauri API wrapper"
```

---

### Task 8: Build the Svelte Stores

**Files:**
- Create: `src/lib/stores/spaces.ts`
- Create: `src/lib/stores/apps.ts`

**Step 1: Create the spaces store**

Create `src/lib/stores/spaces.ts`:

```typescript
import { writable, derived } from "svelte/store";
import type { SpaceConfig } from "../types";
import * as api from "../api";

export const spaces = writable<SpaceConfig[]>([]);
export const activeSpaceId = writable<string>("general");

export const activeSpace = derived(
  [spaces, activeSpaceId],
  ([$spaces, $activeSpaceId]) =>
    $spaces.find((s) => s.space.id === $activeSpaceId) ?? null
);

export async function loadSpaces() {
  const data = await api.listSpaces();
  spaces.set(data);
  const active = await api.getActiveSpace();
  activeSpaceId.set(active);
}

export async function createNewSpace(name: string) {
  const space = await api.createSpace(name);
  spaces.update((s) => [...s, space]);
}

export async function switchToSpace(spaceId: string) {
  await api.switchSpace(spaceId);
  activeSpaceId.set(spaceId);
  await api.hideAllAppWebviews();
}

export async function deleteExistingSpace(spaceId: string) {
  await api.deleteSpace(spaceId);
  spaces.update((s) => s.filter((sp) => sp.space.id !== spaceId));
  activeSpaceId.set("general");
}

export async function renameExistingSpace(spaceId: string, newName: string) {
  await api.renameSpace(spaceId, newName);
  spaces.update((s) =>
    s.map((sp) =>
      sp.space.id === spaceId
        ? { ...sp, space: { ...sp.space, name: newName } }
        : sp
    )
  );
}
```

**Step 2: Create the apps store**

Create `src/lib/stores/apps.ts`:

```typescript
import { writable } from "svelte/store";
import type { AppConfig } from "../types";
import * as api from "../api";

export const activeAppId = writable<string | null>(null);
export const notificationBadges = writable<Record<string, number>>({});

export async function addNewApp(spaceId: string, name: string, url: string) {
  const app = await api.addApp(spaceId, name, url);
  // Reload spaces to reflect the change (space config includes apps)
  const { loadSpaces } = await import("./spaces");
  await loadSpaces();
  return app;
}

export async function openExistingApp(spaceId: string, appId: string) {
  await api.openApp(spaceId, appId);
  activeAppId.set(appId);
}

export async function switchToExistingApp(spaceId: string, appId: string) {
  await api.switchToApp(spaceId, appId);
  activeAppId.set(appId);
}

export async function closeExistingApp(appId: string) {
  await api.closeApp(appId);
  activeAppId.set(null);
}

export async function removeExistingApp(
  spaceId: string,
  appId: string,
  deleteData: boolean
) {
  await api.removeApp(spaceId, appId, deleteData);
  const { loadSpaces } = await import("./spaces");
  await loadSpaces();
  activeAppId.set(null);
}

export async function reorderExistingApps(
  spaceId: string,
  appIds: string[]
) {
  await api.reorderApps(spaceId, appIds);
  const { loadSpaces } = await import("./spaces");
  await loadSpaces();
}

export function updateBadge(appId: string, count: number) {
  notificationBadges.update((badges) => ({ ...badges, [appId]: count }));
}
```

**Step 3: Commit**

```bash
git add src/lib/stores/
git commit -m "feat: add Svelte stores for spaces and apps state"
```

---

### Task 9: Build the Sidebar UI Components

**Files:**
- Create: `src/lib/components/Sidebar.svelte`
- Create: `src/lib/components/SpaceSwitcher.svelte`
- Create: `src/lib/components/AppItem.svelte`
- Create: `src/lib/components/AddAppDialog.svelte`
- Modify: `src/App.svelte`
- Create: `src/app.css` (if not already present — style overrides)

**Step 1: Create the SpaceSwitcher component**

Create `src/lib/components/SpaceSwitcher.svelte`:

```svelte
<script lang="ts">
  import { spaces, activeSpaceId, switchToSpace, createNewSpace } from "../stores/spaces";

  let showCreateInput = false;
  let newSpaceName = "";

  async function handleCreate() {
    if (newSpaceName.trim()) {
      await createNewSpace(newSpaceName.trim());
      newSpaceName = "";
      showCreateInput = false;
    }
  }
</script>

<div class="space-switcher">
  <select
    value={$activeSpaceId}
    on:change={(e) => switchToSpace(e.currentTarget.value)}
  >
    {#each $spaces as space}
      <option value={space.space.id}>{space.space.name}</option>
    {/each}
  </select>

  <button class="add-space-btn" on:click={() => (showCreateInput = !showCreateInput)} title="New Space">
    +
  </button>

  {#if showCreateInput}
    <div class="create-space-input">
      <input
        bind:value={newSpaceName}
        placeholder="Space name..."
        on:keydown={(e) => e.key === "Enter" && handleCreate()}
        autofocus
      />
      <button on:click={handleCreate}>Create</button>
    </div>
  {/if}
</div>

<style>
  .space-switcher {
    padding: 8px;
    border-bottom: 1px solid var(--border-color, #333);
  }

  select {
    width: calc(100% - 36px);
    padding: 6px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
  }

  .add-space-btn {
    width: 28px;
    height: 28px;
    margin-left: 4px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
    cursor: pointer;
  }

  .create-space-input {
    display: flex;
    gap: 4px;
    margin-top: 6px;
  }

  .create-space-input input {
    flex: 1;
    padding: 4px 6px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
  }

  .create-space-input button {
    padding: 4px 8px;
    background: var(--accent, #4a9eff);
    color: #fff;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }
</style>
```

**Step 2: Create the AppItem component**

Create `src/lib/components/AppItem.svelte`:

```svelte
<script lang="ts">
  import type { AppConfig } from "../types";
  import { activeAppId } from "../stores/apps";
  import { notificationBadges } from "../stores/apps";

  export let app: AppConfig;
  export let onSelect: (app: AppConfig) => void;
  export let onContextMenu: (app: AppConfig, event: MouseEvent) => void;

  $: isActive = $activeAppId === app.id;
  $: badge = $notificationBadges[app.id] ?? 0;
</script>

<button
  class="app-item"
  class:active={isActive}
  on:click={() => onSelect(app)}
  on:contextmenu|preventDefault={(e) => onContextMenu(app, e)}
  title={app.url}
>
  <div class="app-icon">
    {#if app.icon && app.icon !== "auto"}
      <img src={app.icon} alt="" width="24" height="24" />
    {:else}
      <span class="icon-placeholder">{app.name.charAt(0).toUpperCase()}</span>
    {/if}
    {#if badge > 0}
      <span class="badge">{badge > 99 ? "99+" : badge}</span>
    {/if}
  </div>
  <span class="app-name">{app.name}</span>
</button>

<style>
  .app-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px;
    background: transparent;
    color: var(--text-primary, #ccc);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
  }

  .app-item:hover {
    background: var(--bg-hover, #333);
  }

  .app-item.active {
    background: var(--bg-active, #444);
    color: var(--text-primary, #fff);
  }

  .app-icon {
    position: relative;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .app-icon img {
    width: 24px;
    height: 24px;
    border-radius: 4px;
  }

  .icon-placeholder {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent, #4a9eff);
    color: #fff;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 600;
  }

  .badge {
    position: absolute;
    top: -4px;
    right: -4px;
    background: #e74c3c;
    color: #fff;
    font-size: 10px;
    min-width: 16px;
    height: 16px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 3px;
  }

  .app-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
```

**Step 3: Create the AddAppDialog component**

Create `src/lib/components/AddAppDialog.svelte`:

```svelte
<script lang="ts">
  import { fetchSiteInfo } from "../api";

  export let onAdd: (name: string, url: string) => void;
  export let onCancel: () => void;

  let url = "";
  let name = "";
  let loading = false;
  let fetched = false;

  async function handleFetchInfo() {
    if (!url.trim()) return;

    // Ensure URL has protocol
    let normalizedUrl = url.trim();
    if (!normalizedUrl.startsWith("http://") && !normalizedUrl.startsWith("https://")) {
      normalizedUrl = "https://" + normalizedUrl;
      url = normalizedUrl;
    }

    loading = true;
    try {
      const [title, _iconPath] = await fetchSiteInfo(normalizedUrl);
      name = title;
      fetched = true;
    } catch (e) {
      // Use domain as fallback name
      try {
        const parsed = new URL(normalizedUrl);
        name = parsed.hostname;
      } catch {
        name = normalizedUrl;
      }
      fetched = true;
    }
    loading = false;
  }

  function handleSubmit() {
    if (url.trim() && name.trim()) {
      onAdd(name.trim(), url.trim());
    }
  }
</script>

<div class="dialog-overlay" on:click|self={onCancel}>
  <div class="dialog">
    <h3>Add App</h3>

    <label>
      URL
      <div class="url-row">
        <input
          bind:value={url}
          placeholder="https://example.com"
          on:keydown={(e) => e.key === "Enter" && handleFetchInfo()}
        />
        <button on:click={handleFetchInfo} disabled={loading}>
          {loading ? "..." : "Fetch"}
        </button>
      </div>
    </label>

    {#if fetched}
      <label>
        Name
        <input bind:value={name} placeholder="App name" />
      </label>
    {/if}

    <div class="actions">
      <button class="cancel" on:click={onCancel}>Cancel</button>
      <button class="add" on:click={handleSubmit} disabled={!fetched || !name.trim()}>
        Add
      </button>
    </div>
  </div>
</div>

<style>
  .dialog-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .dialog {
    background: var(--bg-primary, #1e1e1e);
    border: 1px solid var(--border-color, #444);
    border-radius: 8px;
    padding: 20px;
    width: 400px;
    max-width: 90%;
  }

  h3 {
    margin: 0 0 16px;
    color: var(--text-primary, #fff);
  }

  label {
    display: block;
    margin-bottom: 12px;
    color: var(--text-secondary, #aaa);
    font-size: 13px;
  }

  input {
    display: block;
    width: 100%;
    margin-top: 4px;
    padding: 8px;
    background: var(--bg-secondary, #2a2a2a);
    color: var(--text-primary, #fff);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
    box-sizing: border-box;
  }

  .url-row {
    display: flex;
    gap: 6px;
  }

  .url-row input {
    flex: 1;
  }

  .url-row button {
    padding: 8px 12px;
    background: var(--accent, #4a9eff);
    color: #fff;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .cancel {
    padding: 8px 16px;
    background: transparent;
    color: var(--text-secondary, #aaa);
    border: 1px solid var(--border-color, #444);
    border-radius: 4px;
    cursor: pointer;
  }

  .add {
    padding: 8px 16px;
    background: var(--accent, #4a9eff);
    color: #fff;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .add:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
```

**Step 4: Create the Sidebar component**

Create `src/lib/components/Sidebar.svelte`:

```svelte
<script lang="ts">
  import SpaceSwitcher from "./SpaceSwitcher.svelte";
  import AppItem from "./AppItem.svelte";
  import AddAppDialog from "./AddAppDialog.svelte";
  import { activeSpace, activeSpaceId } from "../stores/spaces";
  import {
    addNewApp,
    openExistingApp,
    removeExistingApp,
  } from "../stores/apps";
  import type { AppConfig } from "../types";

  let showAddDialog = false;
  let contextMenuApp: AppConfig | null = null;
  let contextMenuPos = { x: 0, y: 0 };

  async function handleAddApp(name: string, url: string) {
    await addNewApp($activeSpaceId, name, url);
    showAddDialog = false;
  }

  async function handleSelectApp(app: AppConfig) {
    await openExistingApp($activeSpaceId, app.id);
  }

  function handleContextMenu(app: AppConfig, event: MouseEvent) {
    contextMenuApp = app;
    contextMenuPos = { x: event.clientX, y: event.clientY };
  }

  async function handleRemoveApp() {
    if (contextMenuApp) {
      await removeExistingApp($activeSpaceId, contextMenuApp.id, false);
      contextMenuApp = null;
    }
  }

  function closeContextMenu() {
    contextMenuApp = null;
  }
</script>

<svelte:window on:click={closeContextMenu} />

<div class="sidebar">
  <SpaceSwitcher />

  <div class="app-list">
    {#if $activeSpace}
      {#each $activeSpace.apps as app (app.id)}
        <AppItem
          {app}
          onSelect={handleSelectApp}
          onContextMenu={handleContextMenu}
        />
      {/each}
    {/if}
  </div>

  <div class="sidebar-footer">
    <button class="add-app-btn" on:click={() => (showAddDialog = true)}>
      + Add App
    </button>
  </div>
</div>

{#if showAddDialog}
  <AddAppDialog
    onAdd={handleAddApp}
    onCancel={() => (showAddDialog = false)}
  />
{/if}

{#if contextMenuApp}
  <div
    class="context-menu"
    style="left: {contextMenuPos.x}px; top: {contextMenuPos.y}px"
  >
    <button on:click={handleRemoveApp}>Remove App</button>
  </div>
{/if}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100%;
    background: var(--bg-primary, #1a1a1a);
    color: var(--text-primary, #ccc);
    overflow: hidden;
  }

  .app-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }

  .sidebar-footer {
    padding: 8px;
    border-top: 1px solid var(--border-color, #333);
  }

  .add-app-btn {
    width: 100%;
    padding: 8px;
    background: transparent;
    color: var(--text-secondary, #888);
    border: 1px dashed var(--border-color, #444);
    border-radius: 6px;
    cursor: pointer;
  }

  .add-app-btn:hover {
    background: var(--bg-hover, #333);
    color: var(--text-primary, #fff);
  }

  .context-menu {
    position: fixed;
    background: var(--bg-primary, #1e1e1e);
    border: 1px solid var(--border-color, #444);
    border-radius: 6px;
    padding: 4px;
    z-index: 2000;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .context-menu button {
    display: block;
    width: 100%;
    padding: 6px 12px;
    background: transparent;
    color: var(--text-primary, #ccc);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    text-align: left;
  }

  .context-menu button:hover {
    background: var(--bg-hover, #333);
  }
</style>
```

**Step 5: Update App.svelte**

Replace `src/App.svelte` with:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import { loadSpaces } from "./lib/stores/spaces";

  onMount(async () => {
    await loadSpaces();
  });
</script>

<main>
  <Sidebar />
</main>

<style>
  :root {
    --bg-primary: #1a1a1a;
    --bg-secondary: #2a2a2a;
    --bg-hover: #333;
    --bg-active: #444;
    --text-primary: #e0e0e0;
    --text-secondary: #888;
    --border-color: #333;
    --accent: #4a9eff;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: var(--bg-primary);
    color: var(--text-primary);
  }

  main {
    height: 100vh;
    overflow: hidden;
  }
</style>
```

**Step 6: Verify it builds**

Run:
```bash
cd /home/davidcristobal/prog/rust/webapps
npm run tauri dev
```

Expected: Window opens showing the sidebar with the "General" space selected, an empty app list, and an "Add App" button.

**Step 7: Commit**

```bash
git add src/
git commit -m "feat: add sidebar UI with space switcher, app list, and add-app dialog"
```

---

### Task 10: Wire Up Notification Badges via Title Change Events

**Files:**
- Modify: `src-tauri/src/commands/webviews.rs`
- Modify: `src-tauri/src/main.rs` (if needed)

**Step 1: Add title change listener to webview creation**

In `src-tauri/src/commands/webviews.rs`, modify the `open_app` function. After the line that creates `webview_builder`, add the `on_document_title_changed` callback:

Replace the webview_builder creation section in `open_app` with:

```rust
    let app_id_for_title = app.id.clone();
    let app_handle_for_title = app_handle.clone();

    let webview_builder = WebviewBuilder::new(&label, webview_url)
        .auto_resize()
        .data_directory(data_dir)
        .on_document_title_changed(move |_webview, title| {
            // Parse notification count from title like "(3) Gmail"
            let count = parse_badge_count(&title);
            // Emit event to sidebar
            let _ = app_handle_for_title.emit(
                "title-changed",
                serde_json::json!({
                    "app_id": app_id_for_title,
                    "title": title,
                    "badge": count
                }),
            );
        });
```

Add this helper function at the bottom of the file:

```rust
fn parse_badge_count(title: &str) -> u32 {
    // Match patterns like "(3)", "(99+)", "(12)" at start of title
    if let Some(start) = title.find('(') {
        if let Some(end) = title[start..].find(')') {
            let inner = &title[start + 1..start + end];
            let cleaned = inner.trim_end_matches('+');
            if let Ok(n) = cleaned.parse::<u32>() {
                return n;
            }
        }
    }
    0
}
```

**Step 2: Listen for the event in Svelte**

Update `src/lib/stores/apps.ts` — add an `initTitleListener` function:

```typescript
import { listen } from "@tauri-apps/api/event";

export async function initTitleListener() {
  await listen<{ app_id: string; title: string; badge: number }>(
    "title-changed",
    (event) => {
      updateBadge(event.payload.app_id, event.payload.badge);
    }
  );
}
```

Then call `initTitleListener()` from `src/App.svelte` in the `onMount`:

```typescript
import { initTitleListener } from "./lib/stores/apps";

onMount(async () => {
  await loadSpaces();
  await initTitleListener();
});
```

**Step 3: Verify it compiles**

```bash
cd /home/davidcristobal/prog/rust/webapps
npm run tauri dev
```

**Step 4: Commit**

```bash
git add src-tauri/src/commands/webviews.rs src/lib/stores/apps.ts src/App.svelte
git commit -m "feat: add notification badge support via webview title change events"
```

---

### Task 11: Add Drag-and-Drop Reorder to Sidebar

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/lib/components/AppItem.svelte`

**Step 1: Make AppItem draggable**

In `src/lib/components/AppItem.svelte`, add drag attributes to the button:

Change the `<button>` element to include:

```svelte
<button
  class="app-item"
  class:active={isActive}
  draggable="true"
  on:click={() => onSelect(app)}
  on:contextmenu|preventDefault={(e) => onContextMenu(app, e)}
  on:dragstart={(e) => {
    e.dataTransfer?.setData("text/plain", app.id);
    e.currentTarget.classList.add("dragging");
  }}
  on:dragend={(e) => {
    e.currentTarget.classList.remove("dragging");
  }}
  title={app.url}
>
```

Add this CSS rule:

```css
.app-item.dragging {
  opacity: 0.4;
}
```

**Step 2: Add drop handling to Sidebar**

In `src/lib/components/Sidebar.svelte`, update the `.app-list` div:

```svelte
<div
  class="app-list"
  on:dragover|preventDefault
  on:drop|preventDefault={handleDrop}
>
```

Add the drop handler in the `<script>` tag:

```typescript
import { reorderExistingApps } from "../stores/apps";

async function handleDrop(event: DragEvent) {
  const draggedId = event.dataTransfer?.getData("text/plain");
  if (!draggedId || !$activeSpace) return;

  // Determine drop position based on mouse Y position
  const appList = event.currentTarget as HTMLElement;
  const items = appList.querySelectorAll(".app-item");
  const currentIds = $activeSpace.apps.map((a) => a.id);
  const draggedIndex = currentIds.indexOf(draggedId);

  let dropIndex = currentIds.length;
  for (let i = 0; i < items.length; i++) {
    const rect = items[i].getBoundingClientRect();
    if (event.clientY < rect.top + rect.height / 2) {
      dropIndex = i;
      break;
    }
  }

  // Reorder
  const newOrder = [...currentIds];
  newOrder.splice(draggedIndex, 1);
  newOrder.splice(dropIndex > draggedIndex ? dropIndex - 1 : dropIndex, 0, draggedId);

  await reorderExistingApps($activeSpaceId, newOrder);
}
```

**Step 3: Verify it works**

```bash
cd /home/davidcristobal/prog/rust/webapps
npm run tauri dev
```

Add two or more apps, then drag to reorder them.

**Step 4: Commit**

```bash
git add src/lib/components/
git commit -m "feat: add drag-and-drop reorder for apps in sidebar"
```

---

### Task 12: End-to-End Integration Test

This is a manual verification task to ensure all Phase 1 features work together.

**Step 1: Start the app**

```bash
cd /home/davidcristobal/prog/rust/webapps
npm run tauri dev
```

**Step 2: Verify these scenarios**

1. **Default state:** App launches with a "General" space and an empty app list
2. **Add app:** Click "+ Add App", enter `https://github.com`, click Fetch, verify title auto-populates, click Add. A webview should open showing GitHub.
3. **Add second app:** Add `https://www.wikipedia.org`. It should appear in the sidebar. Clicking it should switch the webview.
4. **Switch apps:** Click between the two apps in the sidebar. Only the selected app's webview should be visible.
5. **Create space:** Create a new space named "Work" via the space switcher. The sidebar should show an empty app list.
6. **Switch spaces:** Switch back to "General". Apps should reappear. Switch to "Work" — empty again.
7. **Remove app:** Right-click an app, select "Remove App". It should disappear.
8. **Persist across restart:** Close the app, reopen it. Spaces and apps should still be there.
9. **Drag reorder:** If 2+ apps exist, drag to reorder. Close and reopen — order should persist.

**Step 3: Fix any issues found**

Address each issue as a focused fix-and-commit cycle.

**Step 4: Final commit**

```bash
git add -A
git commit -m "fix: address integration test issues for Phase 1 MVP"
```

---

### Task 13: Update CLAUDE.md with Actual Project Structure

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update CLAUDE.md**

After everything is built, update `CLAUDE.md` to reflect the actual file structure and any conventions that emerged during development.

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with actual project structure"
```

---

## Summary

| Task | Description | Estimated Steps |
|------|-------------|----------------|
| 1 | Scaffold Tauri + Svelte + TS | 5 |
| 2 | Rust data models and config storage | 7 |
| 3 | Space management IPC commands | 6 |
| 4 | App management IPC commands | 5 |
| 5 | Webview lifecycle commands | 5 |
| 6 | Favicon fetching | 4 |
| 7 | TypeScript types and API wrapper | 3 |
| 8 | Svelte stores | 3 |
| 9 | Sidebar UI components | 7 |
| 10 | Notification badges | 4 |
| 11 | Drag-and-drop reorder | 4 |
| 12 | End-to-end integration test | 4 |
| 13 | Update CLAUDE.md | 2 |
