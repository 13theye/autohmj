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
use crate::services::translate::{TranslationLanguage, TranslationLanguageSlot, TranslationProvider};
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

// ===== Helpers =====

/// Returns the uppercase DeepL-style language code for a TranslationLanguage.
fn deeplx_code(language: &TranslationLanguage) -> &'static str {
    match language {
        TranslationLanguage::English => "EN",
        TranslationLanguage::Spanish => "ES",
        TranslationLanguage::French => "FR",
        TranslationLanguage::Korean => "KO",
    }
}

/// Strips markdown code fences and surrounding whitespace to extract the JSON object.
fn extract_json(s: &str) -> &str {
    let start = s.find('{').unwrap_or(0);
    let end = s.rfind('}').map(|i| i + 1).unwrap_or(s.len());
    &s[start..end]
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
                let codes: Vec<&'static str> = slots
                    .iter()
                    .map(|(_, lang)| deeplx_code(lang))
                    .collect();

                // Build JSON template: {"EN":"...","ES":"..."}
                let json_template = format!(
                    "{{{}}}",
                    codes.iter()
                        .map(|c| format!("\"{}\":\"...\"", c))
                        .collect::<Vec<_>>()
                        .join(",")
                );

                // Conversation history formatted as "author: message"
                let conversation = history
                    .iter()
                    .map(|(author, msg)| format!("{}: {}", author, msg))
                    .collect::<Vec<_>>()
                    .join("\n");

                // Single user turn: translator prompt + conversation + instructions
                let user_content = format!(
                    "{}\n\n{}\n---\nTranslate the last message into: {}\nRespond with JSON only: {}",
                    prompt,
                    conversation,
                    codes.join(", "),
                    json_template,
                );

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
                                    let code = deeplx_code(language);
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
