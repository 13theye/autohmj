// src/gemma/mod.rs
//
// GemmaProvider: implements AIProvider for the Google Gemini HTTP API

pub mod types;

use reqwest::Client;
use std::error::Error;
use tokio::sync::{broadcast, mpsc};

use crate::services::ai::{AIContext, AIProvider, ProviderOutput};
use crate::settings::GemmaProviderConfig;
use types::*;

pub struct GemmaProvider {
    pub api_key: String,
    pub url: String,
    pub model: String,
    pub thinking_model: bool,
    pub client: Client,
    pub request_timeout: std::time::Duration,
}

impl GemmaProvider {
    pub fn new(config: &GemmaProviderConfig) -> Self {
        println!("Starting GemmaProvider...");

        // Gemma 3 models don't support thinkingConfig; ignore the flag for them.
        let thinking_model = config.thinking_model && !config.model.contains("gemma-3");

        Self {
            api_key: config.api_key.clone(),
            url: config.url.clone(),
            model: config.model.clone(),
            thinking_model,
            client: Client::new(),
            request_timeout: std::time::Duration::from_secs(config.request_timeout_secs),
        }
    }
}

impl AIProvider for GemmaProvider {
    fn spawn_request(
        &self,
        context: AIContext,
        tx: mpsc::Sender<Option<ProviderOutput>>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let contents: Vec<RequestContent> = match context {
            AIContext::Persona(ctx) => {
                let mut parts = vec![RequestContent {
                    role: "user".to_string(),
                    parts: vec![Part { text: ctx.instructions, ..Default::default() }],
                }];
                for msg in &ctx.history {
                    let is_self = msg.author == ctx.persona_id;
                    let role = if is_self { "model" } else { "user" };
                    let text = if is_self {
                        msg.content.clone()
                    } else {
                        format!("{}: {}", msg.author, msg.content)
                    };
                    parts.push(RequestContent {
                        role: role.to_string(),
                        parts: vec![Part { text, ..Default::default() }],
                    });
                }
                if let Some(new_msg) = ctx.new_item {
                    parts.push(RequestContent {
                        role: "user".to_string(),
                        parts: vec![Part {
                            text: format!("{}: {}", new_msg.author, new_msg.content),
                            ..Default::default()
                        }],
                    });
                }
                parts
            }
            AIContext::Moderator(ctx) => {
                let mut parts = vec![RequestContent {
                    role: "user".to_string(),
                    parts: vec![Part { text: ctx.instructions, ..Default::default() }],
                }];
                for msg in &ctx.history {
                    parts.push(RequestContent {
                        role: "user".to_string(),
                        parts: vec![Part {
                            text: format!("{}: {}", msg.author, msg.content),
                            ..Default::default()
                        }],
                    });
                }
                parts
            }
        };
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let url = self.url.clone();
        let thinking_model = self.thinking_model;
        let request_timeout = self.request_timeout;

        rthandle.spawn(async move {
            println!("Gemma async task created");

            let shutdown = async {
                let _ = shutdown_rx.recv().await;
            };

            let task = async {
                match tokio::time::timeout(
                    request_timeout,
                    generate_response(contents, model, url, client, api_key, thinking_model),
                )
                .await
                {
                    Err(_) => {
                        eprintln!("Gemma: Request timed out after {:?}", request_timeout);
                        let _ = tx.send(None).await;
                    }
                    Ok(Ok(response)) => {
                        println!("Received successful response of length {}", response.len());
                        let _ = tx.send(Some(ProviderOutput { message: response, response_id: None })).await;
                    }
                    Ok(Err(e)) => {
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

// Sends a REST API request to Google Gemini
pub async fn generate_response(
    contents: Vec<RequestContent>,
    model: String,
    base_url: String,
    client: Client,
    api_key: String,
    thinking_model: bool,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let url = format!(
        "{base_url}/{model}:generateContent?key={key}",
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
            thinking_config: thinking_model.then_some(ThinkingConfig { include_thoughts: true }),
        },
    };

    let http_response = client.post(&url).json(&request).send().await?;
    let response_text = http_response.text().await?;
    let response: GemmaRawResponse = serde_json::from_str(&response_text)?;

    if let Some(candidate) = response.candidates.first() {
        let answer: String = candidate.content.parts.iter()
            .filter(|p| p.thought != Some(true))
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("");
        if !answer.is_empty() {
            return Ok(answer);
        }
    }

    Err("No response generated".into())
}
