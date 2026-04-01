//! OpenAI API module
//!
//! Implements AIProvider for the OpenAI Chat Completions API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

use crate::services::ai::{AIContext, AIProvider};
use crate::settings::OpenAIProviderConfig;

pub struct OpenAIProvider {
    model: String,
    client: Client,
    url: String,
    api_key: String,
}

impl OpenAIProvider {
    pub fn new(config: &OpenAIProviderConfig) -> Self {
        println!("Starting OpenAIProvider...");
        Self {
            model: config.model.clone(),
            client: Client::new(),
            url: config.url.trim_end_matches('/').to_owned(),
            api_key: config.api_key.clone().unwrap_or_default(),
        }
    }
}

// ===== Chat Completions API types =====

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

// ===== AIProvider impl =====

impl AIProvider for OpenAIProvider {
    fn spawn_request(
        &self,
        context: AIContext,
        tx: mpsc::Sender<Option<String>>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let model = self.model.clone();
        let client = self.client.clone();
        let url = format!("{}/chat/completions", self.url);
        let api_key = self.api_key.clone();

        let (system, input) = match context {
            AIContext::Persona(ctx) => {
                let mut lines: Vec<String> = ctx
                    .history
                    .iter()
                    .map(|msg| format!("{}: {}", msg.author, msg.content))
                    .collect();
                if let Some(new_msg) = ctx.new_item {
                    lines.push(format!("{}: {}", new_msg.author, new_msg.content));
                }
                (ctx.instructions, lines.join("\n"))
            }
            AIContext::Moderator(ctx) => {
                let lines: Vec<String> = ctx
                    .history
                    .iter()
                    .map(|msg| format!("{}: {}", msg.author, msg.content))
                    .collect();
                (ctx.instructions, lines.join("\n"))
            }
        };

        rthandle.spawn(async move {
            let shutdown = async {
                let _ = shutdown_rx.recv().await;
            };

            let task = async {
                let request = ChatRequest {
                    model,
                    messages: vec![
                        ChatMessage { role: "system".to_owned(), content: system },
                        ChatMessage { role: "user".to_owned(), content: input },
                    ],
                };

                println!("OpenAIProvider: sending request");

                let response = client
                    .post(&url)
                    .bearer_auth(&api_key)
                    .json(&request)
                    .send()
                    .await;

                match response {
                    Ok(resp) => match resp.json::<ChatResponse>().await {
                        Ok(chat_resp) => {
                            println!("OpenAIProvider: received response");
                            let message = chat_resp
                                .choices
                                .into_iter()
                                .next()
                                .map(|c| c.message.content);
                            let _ = tx.send(message).await;
                        }
                        Err(e) => {
                            eprintln!("OpenAIProvider: failed to parse response: {}", e);
                            let _ = tx.send(None).await;
                        }
                    },
                    Err(e) => {
                        eprintln!("OpenAIProvider: HTTP error: {}", e);
                        let _ = tx.send(None).await;
                    }
                }
            };

            tokio::select! {
                _ = shutdown => {
                    println!("...OpenAIProvider received shutdown signal");
                }
                _ = task => {
                    println!("...OpenAIProvider task completed normally");
                }
            }
        });
    }
}
