// src/services/gemma_manager.rs
//
// Handle multiple Gemma Personalities' requests and responses

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, mpsc};

use crate::{
    config::GemmaConfig,
    models::{History, HistoryItem},
};

pub struct GemmaManager {
    pub instances: HashMap<GemmaPersona, GemmaInstance>, //<id, GemmaInstance>
    responses: VecDeque<GemmaResponse>,                  // collected responses from Gemma API

    // System prompt
    system_prompt: String,

    // Tokio runtime
    runtime: Option<tokio::runtime::Runtime>,

    // Reqwest client
    client: Client,

    // API common
    api_key: String,

    // Shutdown channel
    shutdown_tx: broadcast::Sender<()>,
}

impl GemmaManager {
    pub fn new(config: &GemmaConfig, api_key: &str) -> Self {
        let system_prompt = config.system.prompt.to_owned();
        let runtime =
            tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime for GemmaManager");
        let (shutdown_tx, _) = broadcast::channel(1);
        let instances = make_personas_from_config(config);

        Self {
            instances,
            responses: VecDeque::new(),
            system_prompt,
            runtime: Some(runtime),
            client: Client::new(),
            api_key: api_key.to_owned(),
            shutdown_tx,
        }
    }

    pub fn send(&mut self, new_item: &HistoryItem, history: &History) -> Result<(), String> {
        // Determine who should speak next
        let gemma_instance = self.instances.get(&self.next_instance()).unwrap();

        // Clone the client, api key, model name
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = gemma_instance.model.clone();

        // Generate API request content
        let contents = gemma_instance.generate_contents(new_item, history, &self.system_prompt);

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
                            println!("Received successful response of length {}", response.len());
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
    }

    // Collect Gemma responses into the queue for exposure to Main
    pub fn receive_all(&mut self) {
        for instance in self.instances.values_mut() {
            match instance.rx.try_recv() {
                Ok(Some(message)) => {
                    println!("{}: {}", instance.id, message);
                    self.responses.push_back(GemmaResponse {
                        author: instance.id.to_owned(),
                        message,
                    });
                }
                Ok(None) => {
                    println!("Received empty response from Gemma task: {}", instance.id);
                }
                Err(_) => {} // ignore
            }
        }
    }

    // Add a new Gemma Instance to the Manager
    pub fn add(&mut self, persona: GemmaPersona, instance: GemmaInstance) {
        self.instances.insert(persona, instance);
    }

    // Pop the first entry in the queue of reponses
    pub fn responses_pop_front(&mut self) -> Option<GemmaResponse> {
        self.responses.pop_front()
    }

    pub fn has_queued_responses(&self) -> bool {
        !self.responses.is_empty()
    }

    // A simple determination of next speaker, for now
    fn next_instance(&self) -> GemmaPersona {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let num = time % 2;
        println!("Number: {}", num);
        match num {
            0 => GemmaPersona::Gemma1,
            1 => GemmaPersona::Gemma2,
            _ => GemmaPersona::Gemma1,
        }
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
pub fn make_personas_from_config(config: &GemmaConfig) -> HashMap<GemmaPersona, GemmaInstance> {
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

    personas.insert(GemmaPersona::Gemma1, gemma1);
    personas.insert(GemmaPersona::Gemma2, gemma2);

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

    fn build_prompt(&self, system_prompt: &str) -> String {
        system_prompt.to_owned() + " Personality: " + self.personality()
    }

    // generate request content to be sent to this instance
    pub fn generate_contents(
        &self,
        new_item: &HistoryItem,
        history: &History,
        system_prompt: &str,
    ) -> Vec<RequestContent> {
        // attach prompt to the message content
        let mut contents = vec![RequestContent {
            role: "user".to_string(),
            parts: vec![Part {
                text: self.build_prompt(system_prompt),
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

            // Prepend human or ai if role is user so AI can differentiate User messages
            let text = if role == "user" {
                format!("{}: {}", item.author, item.message)
            } else {
                item.message.to_owned()
            };

            // attach message body
            contents.push(RequestContent {
                role: role.to_owned(),
                parts: vec![Part { text }],
            });
        }

        // Add current message
        contents.push(RequestContent {
            role: "user".to_owned(),
            parts: vec![Part {
                text: format!("{}: {}", new_item.author, new_item.message),
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

#[derive(Hash, PartialEq, Eq)]
pub enum GemmaPersona {
    Gemma1,
    Gemma2,
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
