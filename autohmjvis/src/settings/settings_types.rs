// src/config/types.rs
//
// Config types for the app

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GoogleConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct GridConfig {
    pub font_size_text: u32,
    pub font_size_translation: u32,
    pub grid_rows: u32,
    pub grid_cols: u32,
    pub top_margin: u32,
    pub bottom_margin: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub grid_spacing: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub grid_line_stroke: u32,
}

#[derive(Debug, Deserialize)]
pub struct AudienceWindowConfig {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
pub struct PerformerWindowConfig {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
pub struct OscSendConfig {
    pub target_addr: String,
    pub target_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct PathConfig {
    pub output_directory: String,
    pub auth: String,
    pub gemma: String,
    pub intro_image: String,
}

#[derive(Debug, Deserialize)]
pub struct PersonaConfig {
    pub id: String,
    pub model: String,
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct RenderMainConfig {
    pub texture_width: u32,
    pub texture_height: u32,
    pub texture_samples: u32,
    pub arc_resolution: u32,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct TempoConfig {
    pub bpm: u32,
}

#[derive(Debug, Deserialize)]
pub struct SystemPromptConfig {
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct TranslationConfig {
    pub enabled: bool,
}
