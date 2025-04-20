// src/config/types.rs
//
// Config types for the app

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub port: f32,
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct WindowConfig {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Deserialize)]
pub struct SpeedConfig {
    pub bpm: u32,
}

#[derive(Debug, Deserialize)]
pub struct OscConfig {
    pub rx_port: u16,
}
