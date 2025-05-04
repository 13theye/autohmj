// src/services/gemma_manager.rs
//
// Handle multiple Gemma Personalities' requests and responses

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    error::Error,
};
use tokio::sync::{broadcast, mpsc};

use crate::{config::GemmaConfig, models::History};

pub struct GemmaManager {
    runtime: Option<tokio::runtime::Runtime>,
    pub instances: HashMap<String, GemmaInstance>, //<id, GemmaInstance>
    responses: VecDeque<GemmaResponse>,            // collected responses from Gemma API

    // Reqwest client
    client: Client,

    // API common
    api_key: String,

    // Shutdown channel
    shutdown_tx: broadcast::Sender<()>,
}

impl GemmaManager {
    pub fn new(config: &GemmaConfig, api_key: &str) -> Self {
        let runtime =
            tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime for GemmaManager");
        let (shutdown_tx, _) = broadcast::channel(1);
        let instances = make_personas_from_config(config);

        Self {
            runtime: Some(runtime),
            instances,
            responses: VecDeque::new(),
            client: Client::new(),
            api_key: api_key.to_owned(),
            shutdown_tx,
        }
    }

    pub fn send(&mut self, gemma_id: &str, message: &str, history: &History) -> Result<(), String> {
        if let Some(gemma_instance) = self.instances.get(gemma_id) {
            // Clone the client, api key, model name
            let client = self.client.clone();
            let api_key = self.api_key.clone();
            let model = gemma_instance.model.clone();

            // Generate API request content
            let contents = gemma_instance.generate_contents(message, history);

            if let Some(runtime) = &self.runtime {
                let tx = gemma_instance.tx.clone();
                let mut shutdown_rx = self.shutdown_tx.subscribe();
                runtime.spawn(async move {
                    println!("Gemma async task created");

                    let shutdown = async {
                        let _ = shutdown_rx.recv().await;
                    };

                    let task = async {
                        match generate_response(contents, model, client, api_key).await {
                            Ok(response) => {
                                println!(
                                    "Received successful response of length {}",
                                    response.len()
                                );
                                // Pass message to Instance
                                let _ = tx.send(Some(response)).await;
                            }
                            Err(e) => {
                                eprintln!("Gemma API error: {}", e);
                                if let Some(source) = e.source() {
                                    eprintln!("Error source: {}", source);
                                }
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

            Ok(())
        } else {
            Err(format!("No Gemma instance found with id: {}", gemma_id))
        }
    }

    // Collect Gemma responses into the queue for exposure to Main
    pub fn receive_all(&mut self) {
        for instance in self.instances.values_mut() {
            if let Ok(Some(message)) = instance.rx.try_recv() {
                println!("{}: {}", instance.id, message);
                let response = GemmaResponse {
                    author: instance.id.to_owned(),
                    message,
                };
                self.responses.push_back(response);
            }
        }
    }

    // Add a new Gemma Instance to the Manager
    pub fn add(&mut self, instance: GemmaInstance) {
        self.instances.insert(instance.id.clone(), instance);
    }

    // Pop the first entry in the queue of reponses
    pub fn responses_pop_front(&mut self) -> Option<GemmaResponse> {
        self.responses.pop_front()
    }

    pub fn has_queued_responses(&self) -> bool {
        !self.responses.is_empty()
    }

    /**************************** Shutdown **************************************/

    pub fn shutdown(&mut self) {
        println!("...Shutting down GemmaManager...");

        // Signal all tasks to terminate
        let _ = self.shutdown_tx.send(());

        // Take ownership of the runtime
        if let Some(runtime) = self.runtime.take() {
            // Shut down runtime from a separate thread to avoid blocking
            std::thread::spawn(move || {
                println!(".....Shutting down Gemma runtime in separate thread...");
                runtime.shutdown_timeout(std::time::Duration::from_secs(1));
            })
            .join()
            .ok();
            println!(".....Gemma runtime shutdown successfully");
        }
    }
}

impl Drop for GemmaManager {
    fn drop(&mut self) {
        println!("...GemmaManager being dropped");
        self.shutdown();
        // Wait briefly
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// an overly specific function to create and add two Gemmas from the config file
pub fn make_personas_from_config(config: &GemmaConfig) -> HashMap<String, GemmaInstance> {
    let mut personas = HashMap::new();

    let gemma1 = GemmaInstance::new(
        &config.persona_1.id,
        Personality {
            prompt: config.persona_1.prompt.to_owned(),
        },
        &config.persona_1.model,
    );
    println!("Gemma1 ID: {:?}", gemma1.id);
    let gemma2 = GemmaInstance::new(
        &config.persona_2.id,
        Personality {
            prompt: config.persona_2.prompt.to_owned(),
        },
        &config.persona_2.model,
    );

    println!("Gemma2 ID: {:?}", gemma2.id);

    personas.insert(gemma1.id.to_owned(), gemma1);
    personas.insert(gemma2.id.to_owned(), gemma2);

    personas
}

// Sends a Rest API request to Google Gemini
async fn generate_response(
    contents: Vec<RequestContent>,
    model: String,
    client: Client,
    api_key: String,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    // Build URL
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/{model}:generateContent?key={key}",
        model = model,
        key = api_key,
    );

    // Build request with default values
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

    println!("\nRequest to gemma: {:#?}", request);

    let http_response = client.post(&url).json(&request).send().await?;

    // Log the raw response body before parsing
    let response_text = http_response.text().await?;
    println!("\nRaw response: {}", response_text);

    let response: GemmaRawResponse = serde_json::from_str(&response_text)?;

    // Extract text from the first candidate's content
    if let Some(candidate) = response.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            return Ok(part.text.clone());
        }
    }

    Err("No response generated".into())
}

#[derive(Debug)]
pub struct GemmaInstance {
    pub id: String,
    personality: Personality,
    model: String,
    tx: mpsc::Sender<Option<String>>,
    rx: mpsc::Receiver<Option<String>>,
}

impl GemmaInstance {
    pub fn new(id: &str, personality: Personality, model: &str) -> Self {
        let (tx, rx) = mpsc::channel(16);
        Self {
            id: id.to_owned(),
            personality,
            model: model.to_owned(),
            tx,
            rx,
        }
    }

    // retrieve a reference to the prompt
    pub fn personality(&self) -> &str {
        &self.personality.prompt
    }

    // generate request content to be sent to this instance
    pub fn generate_contents(&self, message: &str, history: &History) -> Vec<RequestContent> {
        // attach prompt to the message content
        let mut contents = vec![RequestContent {
            role: "user".to_string(),
            parts: vec![Part {
                text: self.personality().to_owned(),
            }],
        }];

        // Add conversation history
        for item in history.values() {
            // assign role according to message author
            let role = if item.author == self.id {
                "model"
            } else {
                "user"
            };

            // attach message body
            contents.push(RequestContent {
                role: role.to_owned(),
                parts: vec![Part {
                    text: item.message.to_owned(),
                }],
            });
        }

        // Add current message
        contents.push(RequestContent {
            role: "user".to_owned(),
            parts: vec![Part {
                text: message.to_owned(),
            }],
        });

        contents
    }
}

// A wrapper for a prompt defining a Gemma personality
#[derive(Debug)]
pub struct Personality {
    pub prompt: String,
}

// A parsed response from a Gemma persona
pub struct GemmaResponse {
    pub author: String,
    pub message: String,
}

/********************* API types ************************************* */

// The actual request contents to send to the Gemini API
#[derive(Debug, Serialize)]
struct GemmaRequest {
    contents: Vec<RequestContent>,
    safety_settings: Vec<SafetySetting>,
    generation_config: GenerationConfig,
}

// Safety Settings, of the content as defined by Gemini API
#[derive(Debug, Serialize)]
struct SafetySetting {
    category: String,
    threshold: String,
}

// Config settings, of the content as defined by Gemini API
#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    max_output_tokens: u32,
    top_p: f32,
    top_k: u32,
}

// Request content as defined by Gemini API
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RequestContent {
    role: String,
    parts: Vec<Part>,
}

// Part of the content as defined by Gemini API
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Part {
    text: String,
}

// The Gemma raw response as definied by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct GemmaRawResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(default)]
    prompt_feedback: Option<PromptFeedback>,
    #[serde(default)]
    usage_metadata: Option<UsageMetadata>,
}

// Semantic content candidate as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct Candidate {
    content: RequestContent,
    #[serde(default)]
    safety_ratings: Vec<SafetyRating>,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    index: Option<i32>,
    #[serde(default)]
    token_count: Option<i32>,
}

// Safety metadata, part of the response as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SafetyRating {
    category: String,
    probability: String,
}

// Metadata, part of the response as defined by Gemini API
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct PromptFeedback {
    #[serde(default)]
    block_reason: Option<String>,
    #[serde(default)]
    safety_ratings: Option<Vec<SafetyRating>>,
}

// Metadata, part of the response as defined by Gemini API
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
