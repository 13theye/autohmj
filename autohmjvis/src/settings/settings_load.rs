// src/config/config_load.rs
//
// Loads config.toml
//
// Version 1.0-autofolk music
// 15 Apr 2025

use super::settings_types::*;
use config::{Config, ConfigError, File};
use serde::Deserialize;
use std::env;
use toml;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub grid: GridConfig,
    pub audience_window: AudienceWindowConfig,
    pub performer_window: PerformerWindowConfig,
    pub osc_send: OscSendConfig,
    pub paths: PathConfig,
    pub rendering_main: RenderMainConfig,
    pub server: ServerConfig,
    pub tempo: TempoConfig,
    pub translation: TranslationConfig,
    pub ai_provider: AIProviderConfig,
}

#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    pub google: GoogleConfig,
}

impl Settings {
    /************************* Config file loading ********************/

    pub fn load() -> Result<Self, ConfigError> {
        // Get the executable's directory
        let exe_path = env::current_exe()
            .map_err(|e| ConfigError::Message(format!("Failed to get executable path: {}", e)))?;

        let exe_dir = exe_path.parent().ok_or_else(|| {
            ConfigError::Message("Failed to get executable directory".to_string())
        })?;

        // Build path to config file relative to executable
        let config_path = exe_dir.join("autohmjvis-support").join("config");

        let config_path_str = config_path
            .to_str()
            .ok_or_else(|| ConfigError::Message("Invalid config path".to_string()))?;

        let s = Config::builder()
            // Load configuration file from executable's directory
            .add_source(File::with_name(config_path_str).required(true))
            .build()?;

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_deserialize()
    }
}

impl AuthConfig {
    pub fn load(folder_name: &str) -> Result<Self, ConfigError> {
        // Get the executable's directory
        let exe_path = env::current_exe()
            .map_err(|e| ConfigError::Message(format!("Failed to get executable path: {}", e)))?;

        let exe_dir = exe_path.parent().ok_or_else(|| {
            ConfigError::Message("Failed to get executable directory".to_string())
        })?;

        // Build path to config file relative to executable
        let key_path = exe_dir
            .join("autohmjvis-support")
            .join(folder_name)
            .join("key");

        let key_path_str = key_path
            .to_str()
            .ok_or_else(|| ConfigError::Message("Invalid config path".to_string()))?;

        let s = Config::builder()
            // Load configuration file from executable's directory
            .add_source(File::with_name(key_path_str).required(true))
            .build()?;

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_deserialize()
    }
}

pub struct UiStateConfig;

impl UiStateConfig {
    fn support_path() -> Option<std::path::PathBuf> {
        let exe_path = env::current_exe().ok()?;
        let exe_dir = exe_path.parent()?;
        Some(exe_dir.join("autohmjvis-support").join("ui_state.toml"))
    }

    pub fn load() -> UiStateFileConfig {
        let path = match Self::support_path() {
            Some(p) => p,
            None => return UiStateFileConfig::default(),
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return UiStateFileConfig::default(),
        };
        toml::from_str(&content).unwrap_or_default()
    }

    pub fn save(config: &UiStateFileConfig) -> Result<(), String> {
        let path =
            Self::support_path().ok_or_else(|| "Could not determine save path".to_string())?;
        let content = toml::to_string(config).map_err(|e| format!("Serialize error: {}", e))?;
        std::fs::write(&path, content).map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    }
}

impl GemmaProviderConfig {
    pub fn load(folder_name: &str) -> Result<Self, ConfigError> {
        let exe_path = env::current_exe()
            .map_err(|e| ConfigError::Message(format!("Failed to get executable path: {}", e)))?;
        let exe_dir = exe_path.parent().ok_or_else(|| {
            ConfigError::Message("Failed to get executable directory".to_string())
        })?;
        let path = exe_dir
            .join("autohmjvis-support")
            .join(folder_name)
            .join("gemma");
        let path_str = path
            .to_str()
            .ok_or_else(|| ConfigError::Message("Invalid config path".to_string()))?;
        let s = Config::builder()
            .add_source(File::with_name(path_str).required(true))
            .build()?;
        s.try_deserialize()
    }
}

impl LocalAIProviderConfig {
    pub fn load(folder_name: &str) -> Result<Self, ConfigError> {
        let exe_path = env::current_exe()
            .map_err(|e| ConfigError::Message(format!("Failed to get executable path: {}", e)))?;
        let exe_dir = exe_path.parent().ok_or_else(|| {
            ConfigError::Message("Failed to get executable directory".to_string())
        })?;
        let path = exe_dir
            .join("autohmjvis-support")
            .join(folder_name)
            .join("local_ai");
        let path_str = path
            .to_str()
            .ok_or_else(|| ConfigError::Message("Invalid config path".to_string()))?;
        let s = Config::builder()
            .add_source(File::with_name(path_str).required(true))
            .build()?;
        s.try_deserialize()
    }
}
