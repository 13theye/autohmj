// src/services/ai_translation.rs
//
// AI-based translation backend. Uses an OpenAI-compatible Chat Completions
// endpoint to translate a message into all requested languages in a single
// request, returning results as JSON keyed by DeepL-style language codes
// (e.g. "EN", "ES", "FR"). Which languages are requested is determined at
// runtime from the slots passed by TranslationService — respecting the same
// `target_language`, `second_target_language`, and `enable_second_language`
// settings used by DeepLXProvider.

use std::collections::HashMap;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

use crate::services::translate::{
    build_json_template, build_lang_codes, extract_json, format_history, TranslationLanguage,
    TranslationLanguageSlot, TranslationProvider,
};
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

// ===== TranslationProvider impl =====

impl TranslationProvider for AITranslationProvider {
    /// Sends a single Chat Completions request for all active slots.
    ///
    /// The language list and expected JSON format are built dynamically from
    /// `slots`, so the request automatically respects `enable_second_language`,
    /// `target_language`, and `second_target_language` from config without any
    /// extra configuration here.
    ///
    /// Request structure:
    ///   System: base instructions (from config prompt)
    ///   User:   full conversation history
    ///           ---
    ///           Translate the last message into: EN, ES
    ///           Respond with JSON only: {"EN":"...","ES":"..."}
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
                // Build language codes from slots (e.g. ["EN", "ES"])
                let codes = build_lang_codes(&slots);

                // Build JSON template: {"EN":"...","ES":"..."}
                let json_template = build_json_template(&codes);

                // Conversation history formatted as "author: message"
                let conversation = format_history(&history);

                // Append translation instructions after the conversation
                let user_content = format!(
                    "{}\n---\nTranslate the last message into: {}\nRespond with JSON only: {}",
                    conversation,
                    codes.join(", "),
                    json_template,
                );

                let request = ChatRequest {
                    model,
                    messages: vec![
                        ChatMessage { role: "system".to_owned(), content: prompt },
                        ChatMessage { role: "user".to_owned(), content: user_content },
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
                                let json_str = extract_json(&content);
                                match serde_json::from_str::<HashMap<String, String>>(json_str) {
                                    Ok(result) => {
                                        for (slot, language) in &slots {
                                            let code = language.deeplx_code();
                                            let translation = result.get(code).cloned();
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
