// src/services/ai.rs
//
// Handle for AI provider

use autohmjcommon::HistoryItem;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, error::Error};

#[derive(Clone)]
pub struct GemmaInstance {
    pub id: String,
    pub client: Client,
    pub prompt: String,
    api_key: String,
    model: String,
}

#[derive(Clone)]
pub struct GemmaHandle {
    id: String,
    api_key: String,
    model: String,
}

#[derive(Debug, Serialize)]
struct GemmaRequest {
    contents: Vec<Content>,
    safety_settings: Vec<SafetySetting>,
    generation_config: GenerationConfig,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct SafetySetting {
    category: String,
    threshold: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    max_output_tokens: u32,
    top_p: f32,
    top_k: u32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct GemmaResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(default)]
    prompt_feedback: Option<PromptFeedback>,
    #[serde(default)]
    usage_metadata: Option<UsageMetadata>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct Candidate {
    content: Content,
    #[serde(default)]
    safety_ratings: Vec<SafetyRating>,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    index: Option<i32>,
    #[serde(default)]
    token_count: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SafetyRating {
    category: String,
    probability: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct PromptFeedback {
    #[serde(default)]
    block_reason: Option<String>,
    #[serde(default)]
    safety_ratings: Option<Vec<SafetyRating>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct UsageMetadata {
    #[serde(default)]
    prompt_token_count: Option<i32>,
    #[serde(default)]
    candidates_token_count: Option<i32>,
    #[serde(default)]
    total_token_count: Option<i32>,
}

impl GemmaInstance {
    pub fn new(id: String, prompt: String, api_key: String) -> Self {
        Self {
            id,
            api_key,
            model: "models/gemma-3-27b-it".to_string(),
            client: Client::new(),
            prompt,
        }
    }

    pub fn create_handle(&self) -> GemmaHandle {
        GemmaHandle {
            id: self.id.to_owned(),
            api_key: self.api_key.to_owned(),
            model: self.model.to_owned(),
        }
    }
}

pub fn generate_contents(
    self_id: &str,
    message: &str,
    prompt: &str,
    history: &BTreeMap<usize, HistoryItem>,
) -> Vec<Content> {
    // attach prompt to the message content
    let mut contents = vec![Content {
        role: "user".to_string(),
        parts: vec![Part {
            text: prompt.to_owned(),
        }],
    }];

    // Add conversation history
    for item in history.values() {
        let role = if item.author == self_id {
            "model"
        } else {
            "user"
        };

        contents.push(Content {
            role: role.to_owned(),
            parts: vec![Part {
                text: item.msg.to_owned(),
            }],
        });
    }

    // Add current message
    contents.push(Content {
        role: "user".to_owned(),
        parts: vec![Part {
            text: message.to_owned(),
        }],
    });

    contents
}

impl GemmaHandle {
    pub async fn generate_response(
        &self,
        contents: Vec<Content>,
        client: Client,
    ) -> Result<String, Box<dyn Error>> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{model}:generateContent?key={key}",
            model = self.model,
            key = self.api_key,
        );

        let request = GemmaRequest {
            contents,

            safety_settings: vec![SafetySetting {
                category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            }],
            generation_config: GenerationConfig {
                temperature: 0.9,
                max_output_tokens: 1024,
                top_p: 0.95,
                top_k: 40,
            },
        };

        println!("Request to gemma: {:?}", request);

        let http_response = client.post(&url).json(&request).send().await?;

        // Log the raw response body before parsing
        let response_text = http_response.text().await?;
        println!("Raw response: {}", response_text);

        let response: GemmaResponse = serde_json::from_str(&response_text)?;

        // Extract text from the first candidate's content
        println!("Gemma raw response: {:?}", response);
        if let Some(candidate) = response.candidates.first() {
            if let Some(part) = candidate.content.parts.first() {
                return Ok(part.text.clone());
            }
        }

        Err("No response generated".into())
    }
}
