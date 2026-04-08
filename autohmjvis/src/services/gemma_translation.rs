// src/services/gemma_translation.rs
//
// Gemma cloud translation backend. Uses the same Google Gemini API client and
// gemma.toml config (including the thinking model flag) as the main AI provider,
// activated when translation.provider = "gemma" in config.toml.

use std::collections::HashMap;

use reqwest::Client;
use tokio::sync::{broadcast, mpsc};

use crate::gemma::{
    generate_response,
    types::{Part, RequestContent},
};
use crate::services::translate::{
    build_json_template, build_lang_codes, extract_json, format_history, TranslationLanguage,
    TranslationLanguageSlot, TranslationProvider,
};
use crate::settings::GemmaProviderConfig;

pub struct GemmaTranslationProvider {
    client: Client,
    api_key: String,
    url: String,
    model: String,
    thinking_model: bool,
    prompt: String,
}

impl GemmaTranslationProvider {
    pub fn new(config: &GemmaProviderConfig) -> Self {
        println!("Starting GemmaTranslationProvider...");
        // Mirror GemmaProvider: Gemma 3 models don't support thinkingConfig.
        let thinking_model = config.thinking_model && !config.model.contains("gemma-3");
        Self {
            client: Client::new(),
            api_key: config.api_key.clone(),
            url: config.url.clone(),
            model: config.model.clone(),
            thinking_model,
            prompt: config.translator.prompt.clone(),
        }
    }
}

// ===== TranslationProvider impl =====

impl TranslationProvider for GemmaTranslationProvider {
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
        let api_key = self.api_key.clone();
        let url = self.url.clone();
        let model = self.model.clone();
        let thinking_model = self.thinking_model;
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

                // Just the message to be translated:
                let Some(last_message) = history.last().map(|(_, msg)| msg) else {
                    println!("WARNING: No message to translate");
                    return;
                };

                // Single user turn: translator prompt + conversation + instructions
                let user_content = format!(
                    "{}\n---\nConversation history:\n{}\n---\nTranslate the following message into: {}\n---\nMessage: {}\n---\nRespond with JSON only: {}",
                    prompt,
                    conversation,
                    codes.join(", "),
                    last_message,
                    json_template,
                );

                println!("GemmaTranslationProvider: {:#?}", user_content);

                let contents = vec![RequestContent {
                    role: "user".to_string(),
                    parts: vec![Part { text: user_content, ..Default::default() }],
                }];


                match generate_response(contents, model, url, client, api_key, thinking_model).await {
                    Ok(response) => {
                        let json_str = extract_json(&response);
                        match serde_json::from_str::<HashMap<String, String>>(json_str) {
                            Ok(result) => {
                                for (slot, language) in &slots {
                                    let code = language.deeplx_code();
                                    let translation = result.get(code).cloned();
                                    let _ = tx.send((key, translation, *slot)).await;
                                }
                            }
                            Err(e) => {
                                eprintln!("GemmaTranslationProvider: JSON parse error: {} | raw: {}", e, response);
                                for (slot, _) in &slots {
                                    let _ = tx.send((key, None, *slot)).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("GemmaTranslationProvider: API error: {}", e);
                        for (slot, _) in &slots {
                            let _ = tx.send((key, None, *slot)).await;
                        }
                    }
                }
            };

            tokio::select! {
                _ = shutdown => {
                    println!("...GemmaTranslationProvider received shutdown signal");
                }
                _ = task => {
                    println!("...GemmaTranslationProvider task completed normally");
                }
            }
        });
    }
}
