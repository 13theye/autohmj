// src/gemma/mod.rs
//
// GemmaProvider: implements AIProvider for the Google Gemini HTTP API

pub mod types;

use reqwest::Client;
use std::error::Error;
use tokio::sync::{broadcast, mpsc};

use crate::services::ai::{AIMessage, AIProvider, AIRole};
use crate::settings::GemmaProviderConfig;
use types::*;

pub struct GemmaProvider {
    pub api_key: String,
    pub url: String,
    pub model: String,
    pub client: Client,
}

impl GemmaProvider {
    pub fn new(config: &GemmaProviderConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            url: config.url.clone(),
            model: config.model.clone(),
            client: Client::new(),
        }
    }
}

impl AIProvider for GemmaProvider {
    fn spawn_request(
        &self,
        messages: Vec<AIMessage>,
        tx: mpsc::Sender<Option<String>>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let contents = ai_messages_to_request_contents(messages);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let url = self.url.clone();

        rthandle.spawn(async move {
            println!("Gemma async task created");

            let shutdown = async {
                let _ = shutdown_rx.recv().await;
            };

            let task = async {
                match generate_response(contents, model, url, client, api_key).await {
                    Ok(response) => {
                        println!("Received successful response of length {}", response.len());
                        let _ = tx.send(Some(response)).await;
                    }
                    Err(e) => {
                        eprintln!("Gemma API error: {}", e);
                        if let Some(source) = e.source() {
                            eprintln!("Error source: {}", source);
                        }
                        let _ = tx.send(None).await;
                    }
                }
            };

            tokio::select! {
                _ = shutdown => {
                    println!("...Gemma task received shutdown signal");
                }
                _ = task => {
                    println!("Gemma task completed normally")
                }
            }
        });
    }
}

// Convert Vec<AIMessage> to Vec<RequestContent> for the Gemini API.
// User role messages become "user" role (Gemini has no system role).
fn ai_messages_to_request_contents(messages: Vec<AIMessage>) -> Vec<RequestContent> {
    messages
        .into_iter()
        .map(|msg| RequestContent {
            role: match msg.role {
                AIRole::User => "user".to_string(),
                AIRole::Model => "model".to_string(),
            },
            parts: vec![Part { text: msg.content }],
        })
        .collect()
}

// Sends a REST API request to Google Gemini
async fn generate_response(
    contents: Vec<RequestContent>,
    model: String,
    base_url: String,
    client: Client,
    api_key: String,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let url = format!(
        "{base_url}{model}:generateContent?key={key}",
        base_url = base_url,
        model = model,
        key = api_key,
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

    let http_response = client.post(&url).json(&request).send().await?;
    let response_text = http_response.text().await?;
    let response: GemmaRawResponse = serde_json::from_str(&response_text)?;

    if let Some(candidate) = response.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            return Ok(part.text.clone());
        }
    }

    Err("No response generated".into())
}
