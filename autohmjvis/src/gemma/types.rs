// src/gemma/types.rs
//
// Gemini API wire types

use serde::{Deserialize, Serialize};

// The actual request contents to send to the Gemini API
#[derive(Debug, Serialize)]
pub struct GemmaRequest {
    pub contents: Vec<RequestContent>,
    pub safety_settings: Vec<SafetySetting>,
    pub generation_config: GenerationConfig,
}

// Safety Settings of the response content as defined by Gemini API
#[derive(Debug, Serialize)]
pub struct SafetySetting {
    pub category: String,
    pub threshold: String,
}

// Config settings of the response content as defined by Gemini API
#[derive(Debug, Serialize)]
pub struct GenerationConfig {
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub top_p: f32,
    pub top_k: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

// Request content as defined by Gemini API
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RequestContent {
    pub role: String,
    pub parts: Vec<Part>,
}

// Part of the content as defined by Gemini API
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Part {
    pub text: String,
    /// True for thinking/reasoning parts (Gemma 4 thinking models).
    /// Absent (None) for regular answer parts and all Gemma 3 responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
}

// Thinking configuration for thinking models (e.g. Gemma 4)
#[derive(Debug, Serialize)]
pub struct ThinkingConfig {
    pub include_thoughts: bool,
}

// The Gemma raw response as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub struct GemmaRawResponse {
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    pub prompt_feedback: Option<PromptFeedback>,
    #[serde(default)]
    pub usage_metadata: Option<UsageMetadata>,
}

// Semantic content candidate as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub struct Candidate {
    pub content: RequestContent,
    #[serde(default)]
    pub safety_ratings: Vec<SafetyRating>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub index: Option<i32>,
    #[serde(default)]
    pub token_count: Option<i32>,
}

// Safety metadata, part of the response as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
}

// Metadata, part of the response as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub struct PromptFeedback {
    #[serde(default)]
    pub block_reason: Option<String>,
    #[serde(default)]
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

// Metadata, part of the response as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub struct UsageMetadata {
    #[serde(default)]
    pub prompt_token_count: Option<i32>,
    #[serde(default)]
    pub candidates_token_count: Option<i32>,
    #[serde(default)]
    pub total_token_count: Option<i32>,
}
