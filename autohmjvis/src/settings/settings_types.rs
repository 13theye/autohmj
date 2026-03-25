// src/config/types.rs
//
// Config types for the app

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GoogleConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct GridConfig {
    pub font_size_text: u32,
    pub font_size_translation: u32,
    pub font_size_statusbar: u32,
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
    pub margin_line_stroke: u32,
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
    pub ai: String,
    pub intro_image: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PersonaConfig {
    pub id: String,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HumanColorConfig {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for HumanColorConfig {
    fn default() -> Self {
        Self {
            r: 0.486,
            g: 0.706,
            b: 0.702,
            a: 1.0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct UiStateFileConfig {
    pub human_color: HumanColorConfig,
}

#[derive(Debug, Deserialize)]
pub struct TranslationConfig {
    pub enabled: bool,
    pub enable_second_language: bool,
    pub target_language: String,
    pub second_target_language: String,
}

#[derive(Debug, Deserialize)]
pub struct GemmaProviderConfig {
    pub system_prompt: String,
    pub api_key: String,
    pub url: String,
    pub model: String,
    pub persona_1: PersonaConfig,
    pub persona_2: PersonaConfig,
    pub moderator: PersonaConfig,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIProviderConfig {
    pub system_prompt: String,
    pub api_key: Option<String>,
    pub url: String,
    pub model: String,
    pub strict_request_object_adherence: bool,
    pub schema_description: Option<String>,
    pub persona_1: PersonaConfig,
    pub persona_2: PersonaConfig,
    pub moderator: PersonaConfig,
}
