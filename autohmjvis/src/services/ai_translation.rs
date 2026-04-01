// src/services/ai_translation.rs
//
// AI-based translation backend. Uses an OpenAI-compatible Chat Completions
// endpoint to translate a message into all requested languages in a single
// request, returning results as JSON.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

use crate::services::translate::{TranslationLanguage, TranslationLanguageSlot, TranslationProvider};
use crate::settings::OpenAIProviderConfig;

pub struct AITranslationProvider {
    client: Client,
    url: String,
    api_key: String,
    model: String,
    prompt: String,
}

impl AITranslationProvider {
    pub fn new(config: &OpenAIProviderConfig) -> Self {
        println!("Starting AITranslationProvider...");
        Self {
            client: Client::new(),
            url: config.url.trim_end_matches('/').to_owned(),
            api_key: config.api_key.clone().unwrap_or_default(),
            model: config.model.clone(),
            prompt: config.translator.prompt.clone(),
        }
    }
}

// ===== OpenAI Chat Completions API types =====

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

// ===== Expected JSON response from the AI =====

#[derive(Deserialize)]
struct TranslationResult {
    english: Option<String>,
    spanish: Option<String>,
    french: Option<String>,
    korean: Option<String>,
}

fn extract_translation(language: &TranslationLanguage, result: &TranslationResult) -> Option<String> {
    match language {
        TranslationLanguage::English => result.english.clone(),
        TranslationLanguage::Spanish => result.spanish.clone(),
        TranslationLanguage::French => result.french.clone(),
        TranslationLanguage::Korean => result.korean.clone(),
    }
}

// ===== TranslationProvider impl =====

impl TranslationProvider for AITranslationProvider {
    /// Sends a single Chat Completions request for all requested slots.
    /// Uses the full conversation history as context, asking the AI to
    /// translate only the last message.
    fn spawn_translation(
        &self,
        key: usize,
        _msg: String,
        history: Vec<(String, String)>,
        slots: Vec<(TranslationLanguageSlot, TranslationLanguage)>,
        tx: mpsc::Sender<(usize, Option<String>, TranslationLanguageSlot)>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let client = self.client.clone();
        let url = format!("{}/chat/completions", self.url);
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let prompt = self.prompt.clone();

        rthandle.spawn(async move {
            let shutdown = async {
                let _ = shutdown_rx.recv().await;
            };

            let task = async {
                let conversation = history
                    .iter()
                    .map(|(author, msg)| format!("{}: {}", author, msg))
                    .collect::<Vec<_>>()
                    .join("\n");

                let request = ChatRequest {
                    model,
                    messages: vec![
                        ChatMessage { role: "system".to_owned(), content: prompt },
                        ChatMessage { role: "user".to_owned(), content: conversation },
                    ],
                };

                let response = client
                    .post(&url)
                    .bearer_auth(&api_key)
                    .json(&request)
                    .send()
                    .await;

                match response {
                    Ok(resp) => match resp.json::<ChatResponse>().await {
                        Ok(chat_resp) => {
                            let content = chat_resp.choices.into_iter().next().map(|c| c.message.content);

                            if let Some(content) = content {
                                match serde_json::from_str::<TranslationResult>(&content) {
                                    Ok(result) => {
                                        for (slot, language) in &slots {
                                            let translation = extract_translation(language, &result);
                                            let _ = tx.send((key, translation, *slot)).await;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("AITranslationProvider: JSON parse error: {} | raw: {}", e, content);
                                        for (slot, _) in &slots {
                                            let _ = tx.send((key, None, *slot)).await;
                                        }
                                    }
                                }
                            } else {
                                eprintln!("AITranslationProvider: empty response content");
                                for (slot, _) in &slots {
                                    let _ = tx.send((key, None, *slot)).await;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("AITranslationProvider: failed to parse response body: {}", e);
                            for (slot, _) in &slots {
                                let _ = tx.send((key, None, *slot)).await;
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("AITranslationProvider: HTTP error: {}", e);
                        for (slot, _) in &slots {
                            let _ = tx.send((key, None, *slot)).await;
                        }
                    }
                }
            };

            tokio::select! {
                _ = shutdown => {
                    println!("...AITranslationProvider received shutdown signal");
                }
                _ = task => {
                    println!("...AITranslationProvider task completed normally");
                }
            }
        });
    }
}
