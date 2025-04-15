// src/config/types.rs
//
// Config types for the app

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BoardConfig {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub gravity_interval: f32,
    pub lock_delay: f32,
}

#[derive(Debug, Deserialize)]
pub struct RenderConfig {
    pub texture_width: u32,
    pub texture_height: u32,
    pub texture_samples: u32,
    pub arc_resolution: u32,
}

#[derive(Debug, Deserialize)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
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
