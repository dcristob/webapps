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
    NoConfigDir,
}

pub fn config_dir() -> Result<PathBuf, ConfigError> {
    let base = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
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
