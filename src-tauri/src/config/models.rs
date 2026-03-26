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
