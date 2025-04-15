// src/config/config_load.rs
//
// Loads config.toml
//
// Version 1.0
// 7 Apr 2025

use super::config_types::*;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub board: BoardConfig,
    pub paths: PathConfig,
    pub rendering: RenderConfig,
    pub window: WindowConfig,
    pub osc: OscConfig,
    pub frame_recorder: FrameRecorderConfig,
    pub speed: SpeedConfig,
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

    /*
    pub fn resolve_project_path(&self) -> PathBuf {
        if Path::new(&self.paths.project_file).is_absolute() {
            PathBuf::from(&self.paths.project_file)
        } else {
            // If path is relative, resolve it relative to the executable or working directory
            if let Some(exe_dir) = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            {
                exe_dir.join(&self.paths.project_file)
            } else {
                PathBuf::from(&self.paths.project_file)
            }
        }
    }
    */

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
