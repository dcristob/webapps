use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub general: GeneralSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    pub sidebar_width: u32,
    pub theme: String,
    /// Minutes of inactivity before a background app's webview is destroyed to save memory.
    /// 0 = disabled.
    #[serde(default = "default_sleep_timeout")]
    pub sleep_timeout_mins: u32,
    #[serde(default)]
    pub space_order: Vec<String>,
}

fn default_sleep_timeout() -> u32 {
    15
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            general: GeneralSettings {
                sidebar_width: 100,
                theme: "dark".to_string(),
                sleep_timeout_mins: 15,
                space_order: vec![],
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
    #[serde(default = "default_space_color")]
    pub color: String,
    pub isolation: IsolationMode,
}

fn default_space_color() -> String {
    "#4a9eff".to_string()
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PermissionState {
    #[default]
    Ask,
    Allow,
    Block,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Camera,
    Microphone,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPermissions {
    #[serde(default)]
    pub camera: PermissionState,
    #[serde(default)]
    pub microphone: PermissionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub icon: String,
    pub isolation_override: bool,
    #[serde(default)]
    pub permissions: AppPermissions,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_defaults_permissions_to_ask() {
        let toml = r#"
id = "abc"
name = "Test"
url = "https://example.com"
icon = "auto"
isolation_override = false
"#;
        let app: AppConfig = toml::from_str(toml).expect("parse");
        assert_eq!(app.permissions.camera, PermissionState::Ask);
        assert_eq!(app.permissions.microphone, PermissionState::Ask);
    }

    #[test]
    fn app_config_roundtrip_with_permissions() {
        let app = AppConfig {
            id: "abc".to_string(),
            name: "Test".to_string(),
            url: "https://example.com".to_string(),
            icon: "auto".to_string(),
            isolation_override: false,
            permissions: AppPermissions {
                camera: PermissionState::Allow,
                microphone: PermissionState::Block,
            },
        };
        let s = toml::to_string(&app).expect("serialize");
        let back: AppConfig = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.permissions.camera, PermissionState::Allow);
        assert_eq!(back.permissions.microphone, PermissionState::Block);
    }
}
