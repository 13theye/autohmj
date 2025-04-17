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
pub struct InputWindowConfig {
    pub width: u32,
    pub height: u32,
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
}

#[derive(Debug, Deserialize)]
pub struct OscConfig {
    pub rx_port: u16,
}
