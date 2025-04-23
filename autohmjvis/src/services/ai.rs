// src/services/ai.rs
//
// Handle for AI provider

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Clone)]
pub struct Gemma {
    api_key: String,
    client: Client,
    model: String,
}

#[derive(Serialize)]
struct GemmaRequest {
    contents: Vec<Content>,
    safety_settings: Vec<SafetySetting>,
    generation_config: GenerationConfig,
}

#[derive(Deserialize, Serialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Deserialize, Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize)]
struct SafetySetting {
    category: String,
    threshold: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    max_output_tokens: u32,
    top_p: f32,
    top_k: u32,
}

#[derive(Deserialize)]
struct GemmaResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Content,
    safety_ratings: Vec<SafetyRating>,
}

#[derive(Deserialize)]
struct SafetyRating {
    category: String,
    probability: String,
}

impl Gemma {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            model: "models/gemma-3-27b-it".to_string(),
        }
    }

    pub async fn generate_response(&self, message: &str) -> Result<String, Box<dyn Error>> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{model}:generateContent?key={key}",
            model = self.model,
            key = self.api_key
        );

        let request = GemmaRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part {
                    text: message.to_string(),
                }],
            }],
            safety_settings: vec![SafetySetting {
                category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
            }],
            generation_config: GenerationConfig {
                temperature: 0.9,
                max_output_tokens: 1024,
                top_p: 0.95,
                top_k: 40,
            },
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json::<GemmaResponse>()
            .await?;

        // Extract text from the first candidate's content
        if let Some(candidate) = response.candidates.first() {
            if let Some(part) = candidate.content.parts.first() {
                return Ok(part.text.clone());
            }
        }

        Err("No response generated".into())
    }
}
