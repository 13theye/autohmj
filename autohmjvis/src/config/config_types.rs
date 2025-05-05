// src/config/types.rs
//
// Config types for the app

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MainWindowConfig {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub port: u32,
}

#[derive(Debug, Deserialize)]
pub struct RenderMainConfig {
    pub texture_width: u32,
    pub texture_height: u32,
    pub texture_samples: u32,
    pub arc_resolution: u32,
}

#[derive(Debug, Deserialize)]
pub struct FrameRecorderConfig {
    pub frame_limit: u32,
    pub fps: u32,
}

#[derive(Debug, Deserialize)]
pub struct SpeedConfig {
    pub bpm: u32,
}

#[derive(Debug, Deserialize)]
pub struct PathConfig {
    pub output_directory: String,
    pub auth: String,
    pub gemma: String,
}

#[derive(Debug, Deserialize)]
pub struct OscConfig {
    pub rx_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct GoogleConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct SystemPromptConfig {
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct PersonaConfig {
    pub id: String,
    pub model: String,
    pub prompt: String,
}
