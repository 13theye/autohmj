// src/config/config_load.rs
//
// Loads config.toml
//
// Version 1.0-autofolk music
// 15 Apr 2025

use super::config_types::*;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub frame_recorder: FrameRecorderConfig,
    pub grid: GridConfig,
    pub main_window: MainWindowConfig,
    pub osc_send: OscSendConfig,
    pub paths: PathConfig,
    pub rendering_main: RenderMainConfig,
    pub server: ServerConfig,
    pub speed: SpeedConfig,
}

#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    pub google: GoogleConfig,
}

#[derive(Debug, Deserialize)]
pub struct GemmaConfig {
    pub system: SystemPromptConfig,
    pub persona_1: PersonaConfig,
    pub persona_2: PersonaConfig,
}

impl Config {
    /************************* Config file loading ********************/

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // First try to load from the executable's directory
        if let Some(exe_config) = Self::load_from_exe_dir() {
            return Ok(exe_config);
        }

        // Fallback to loading from the current working directory
        Self::load_from_working_dir()
    }

    fn load_from_exe_dir() -> Option<Self> {
        let exe_path = std::env::current_exe().ok()?;
        let exe_dir = exe_path.parent()?;
        let config_path = exe_dir.join("config.toml");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).ok()?;
            toml::from_str(&content).ok()
        } else {
            None
        }
    }

    fn load_from_working_dir() -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string("config.toml")?;
        Ok(toml::from_str(&content)?)
    }

    /************************* Resolving paths to the types needed in app ********************/

    pub fn resolve_output_dir(&self) -> PathBuf {
        if Path::new(&self.paths.output_directory).is_absolute() {
            PathBuf::from(&self.paths.output_directory)
        } else {
            // If path is relative, resolve it relative to the executable or working directory
            if let Some(exe_dir) = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            {
                exe_dir.join(&self.paths.output_directory)
            } else {
                PathBuf::from(&self.paths.output_directory)
            }
        }
    }

    pub fn resolve_output_dir_as_str(&self) -> String {
        let path = if Path::new(&self.paths.output_directory).is_absolute() {
            PathBuf::from(&self.paths.output_directory)
        } else {
            // If path is relative, resolve it relative to the executable or working directory
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .map(|exe_dir| exe_dir.join(&self.paths.output_directory))
                .unwrap_or_else(|| PathBuf::from(&self.paths.output_directory))
        };

        path.to_string_lossy().into_owned() // Convert PathBuf to String safely
    }
}

impl AuthConfig {
    pub fn load(dir: &str) -> Option<Self> {
        let dir_path = if Path::new(dir).is_absolute() {
            PathBuf::from(dir)
        } else {
            // If path is relative, resolve it relative to the executable or working directory
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .map(|exe_dir| exe_dir.join(dir))
                .unwrap_or_else(|| PathBuf::from(dir))
        };

        let path = dir_path.join("key.toml");

        if path.exists() {
            let content = fs::read_to_string(path).ok()?;
            toml::from_str(&content).ok()
        } else {
            None
        }
    }
}

impl GemmaConfig {
    pub fn load(dir: &str) -> Option<Self> {
        let dir_path = if Path::new(dir).is_absolute() {
            PathBuf::from(dir)
        } else {
            // If path is relative, resolve it relative to the executable or working directory
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .map(|exe_dir| exe_dir.join(dir))
                .unwrap_or_else(|| PathBuf::from(dir))
        };

        let path = dir_path.join("gemma.toml");

        if path.exists() {
            let content = fs::read_to_string(path).ok()?;
            toml::from_str(&content).ok()
        } else {
            None
        }
    }
}
