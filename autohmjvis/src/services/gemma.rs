// src/services/gemma_manager.rs
//
// Handle multiple Gemma Personalities' requests and responses

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error};
use tokio::sync::{broadcast, mpsc};

use crate::{
    events::HMJEventBus,
    models::{Conversation, ConvoItem},
    settings::GemmaConfig,
};

// These events are emitted by GemmaService to notify subscribers of responses from the Gemma API.
#[derive(Clone, Debug)]
pub enum GemmaEvent {
    GemmaReceived(GemmaResponse), // A response from a Gemma instance has been received. Also includes the response.
    ModeratorChooses(String),     // The ID of the persona who should speak next
    GemmaRequested(String),       // A request has been sent to a Gemma instance (ID)
}

pub struct GemmaService {
    pub instances: HashMap<GemmaPersona, GemmaInstance>, //<id, GemmaInstance>
    pub moderator: GemmaModerator, // a special instance of Gemma that decides who should speak next

    // System prompt for all Gemma instances
    system_prompt: String,

    // Tokio handle for async tasks
    rthandle: tokio::runtime::Handle,

    // Reqwest client
    client: Client,

    // API common
    api_key: String,

    // Events channel
    event_tx: broadcast::Sender<GemmaEvent>,

    // Shutdown channel
    shutdown_tx: broadcast::Sender<()>,
}

impl GemmaService {
    pub fn new(
        config: &GemmaConfig,
        api_key: &str,
        events: &HMJEventBus,
        rthandle: tokio::runtime::Handle,
    ) -> Self {
        let system_prompt = config.system.prompt.to_owned();
        let (shutdown_tx, _) = broadcast::channel(1);
        let (instances, moderator_instance) = make_instances_from_config(config);
        let moderator = GemmaModerator::from_gemma_instance(moderator_instance);
        let event_tx = events.gemma.clone();

        Self {
            instances,
            moderator,
            system_prompt,
            rthandle,
            client: Client::new(),
            api_key: api_key.to_owned(),
            event_tx,
            shutdown_tx,
        }
    }

    pub fn update(&mut self) {
        self.receive_all();
    }

    // Send a request to a Gemma instance.
    //
    // If the persona is GemmaPersona::Moderator, the conversation history is sent to the moderator
    // instance to decide who should speak next.
    //
    // If the persona is a regular Gemma instance, the new message is sent to the instance.
    //
    pub fn send(
        &mut self,
        gemma_persona: GemmaPersona, // the persona to send the request to
        new_item: Option<&ConvoItem>, // the new message to send (if sending to a speaker persona)
        conversation: &Conversation, // the conversation history
    ) -> Result<(), String> {
        let gemma_instance: &GemmaInstance = if gemma_persona == GemmaPersona::Moderator {
            &self.moderator.instance
        } else {
            let Some(gemma_instance) = self.instances.get(&gemma_persona) else {
                return Err(format!(
                    "No gemma instance found for persona: {:?}",
                    gemma_persona
                ));
            };
            gemma_instance
        };

        // Format and Generate API request content
        let contents = if gemma_persona == GemmaPersona::Moderator {
            self.moderator.generate_contents(conversation)
        } else {
            gemma_instance.generate_contents(new_item, conversation, &self.system_prompt)
        };

        // Clone the client, api key, model name
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = gemma_instance.model.clone();

        // Spawn a new async task to send the request to the Gemma API.
        let tx = gemma_instance.tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Emit a notification that an AI request has been sent
        let _ = self
            .event_tx
            .send(GemmaEvent::GemmaRequested(gemma_instance.id.to_owned()));

        self.rthandle.spawn(async move {
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

        Ok(())
    }

    // Collect Gemma responses and broadcast them to the EventBus.
    //
    // IMPORTANT: currently, the moderator assumes that there is only one human in the conversation
    // Although the client/server can handle multiple humans, support for multiple humans is incomplete.
    pub fn receive_all(&mut self) {
        // Receive from moderator
        match self.moderator.instance.rx.try_recv() {
            Ok(Some(message)) => {
                if message.contains("Human") {
                    let _ = self
                        .event_tx
                        .send(GemmaEvent::ModeratorChooses("Human".to_string()));
                } else {
                    self.instances
                        .values()
                        .filter(|instance| message.contains(&instance.id))
                        .for_each(|instance| {
                            let _ = self
                                .event_tx
                                .send(GemmaEvent::ModeratorChooses(instance.id.clone()));
                        });
                }
            }
            Ok(None) => {
                println!("Received empty response from moderator");
                let _ = self
                    .event_tx
                    .send(GemmaEvent::ModeratorChooses("No Response".to_string()));
            }
            Err(_) => {} // ignore
        }

        // Collect responses from all Gemma instances
        for instance in self.instances.values_mut() {
            match instance.rx.try_recv() {
                Ok(Some(message)) => {
                    //println!("{}: {}", instance.id, message);
                    let _ = self.event_tx.send(GemmaEvent::GemmaReceived(GemmaResponse {
                        author: instance.id.to_owned(),
                        message,
                    }));
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

    /**************************** Shutdown **************************************/

    pub fn shutdown(&mut self) {
        println!("...Shutting down GemmaService...");

        // Signal all tasks to terminate
        let _ = self.shutdown_tx.send(());
    }
}

impl Drop for GemmaService {
    fn drop(&mut self) {
        println!("...GemmaService being dropped");
        self.shutdown();
        // Wait briefly
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// an overly specific function to create and add two Gemmas from the config file
// and a moderator
pub fn make_instances_from_config(
    config: &GemmaConfig,
) -> (HashMap<GemmaPersona, GemmaInstance>, GemmaInstance) {
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

    let moderator = GemmaInstance::new(
        &config.moderator.id,
        Personality {
            prompt: config.moderator.prompt.to_owned(),
        },
        &config.moderator.model,
    );
    println!("Moderator ID: {:?}", moderator.id);

    (personas, moderator)
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

    //println!("\nRequest to gemma: {:#?}", request);

    let http_response = client.post(&url).json(&request).send().await?;

    // Log the raw response body before parsing
    let response_text = http_response.text().await?;
    //println!("\nRaw response: {}", response_text);

    let response: GemmaRawResponse = serde_json::from_str(&response_text)?;

    // Extract text from the first candidate's content
    if let Some(candidate) = response.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            return Ok(part.text.clone());
        }
    }

    Err("No response generated".into())
}

// A GemmaInstance is a single instance of a Gemma persona.
// It is used to send and receive messages to and from the Gemma API.
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

    // build the prompt for this instance
    // the prompt includes the general system prompt and the specific personality prompt.
    fn build_prompt(&self, system_prompt: &str) -> String {
        system_prompt.to_owned() + " Personality: " + self.personality()
    }

    // generate request content to be sent to this instance
    pub fn generate_contents(
        &self,
        // new_item is used when messages were automatically sent to the AI upon pressing
        // enter, because the new item was not part of the conversation history.
        // Now, we only send the convo when performer presses the AI button, after already
        // entering the new_item into the conversation history.
        new_item: Option<&ConvoItem>,
        //
        conversation: &Conversation,
        system_prompt: &str,
    ) -> Vec<RequestContent> {
        // attach prompt to the message contents
        let mut contents = vec![RequestContent {
            role: "user".to_string(),
            parts: vec![Part {
                text: self.build_prompt(system_prompt),
            }],
        }];

        // Add conversation history
        for item in conversation.values() {
            // assign role according to message author
            let role = if item.author == self.id {
                "model"
            } else {
                "user"
            };

            // Prepend human or ai if role is user so AI can differentiate User messages
            let text = if role == "user" {
                format_message(&item.author, &item.message)
            } else {
                item.message.to_owned()
            };

            // attach message body
            contents.push(RequestContent {
                role: role.to_owned(),
                parts: vec![Part { text }],
            });
        }

        // Add the new_item to the contents if it exists.
        if let Some(new_item) = new_item {
            // Add current message
            contents.push(RequestContent {
                role: "user".to_owned(),
                parts: vec![Part {
                    text: format_message(&new_item.author, &new_item.message),
                }],
            });
        }

        contents
    }
}

/********************* Helper functions for GemmaInstance ************************************* */

// Format to include the author of the message
fn format_message(author: &str, message: &str) -> String {
    format!("{}: {}", author, message)
}

/********************* Helper structs ********************************************************* */

// A wrapper for a prompt defining a Gemma personality
#[derive(Debug)]
pub struct Personality {
    pub prompt: String,
}

// A parsed response from a Gemma persona
#[derive(Clone, Debug)]
pub struct GemmaResponse {
    pub author: String,
    pub message: String,
}

// The different Gemma personas.
//
// Gemma1 and Gemma2 are the two regular Gemma instances.
// Moderator is a special instance that decides who should speak next.
#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
pub enum GemmaPersona {
    Gemma1,
    Gemma2,
    Moderator,
}

/********************* AI Moderator ************************************* */
//
// The AI Moderator is a specialized GemmaInstance that receives the complete conversation history
// and responds with a single word: The ID of the persona who should speak next.

#[derive(Debug)]
pub struct GemmaModerator {
    pub instance: GemmaInstance,
}

impl GemmaModerator {
    // Creating a new moderator is similar to creating a regular GemmaInstance, except that the
    // personality prompt contains specific instructions on how to moderate the conversation.
    pub fn new(id: &str, personality: Personality, model: &str) -> Self {
        let instance = GemmaInstance::new(id, personality, model);
        Self { instance }
    }

    // A shortcut for creating a moderator when a moderator instance is already created.
    pub fn from_gemma_instance(instance: GemmaInstance) -> Self {
        Self { instance }
    }

    // Build the moderator prompt
    fn build_prompt(&self) -> String {
        self.instance.personality().to_owned()
    }

    // Generate the contents for the moderator instance.
    //
    // The moderator instance is a special instance that receives the complete conversation history
    // and responds with a single word: The ID of the persona who should speak next.
    //
    // The moderator instance is a special instance that receives the complete conversation history
    // This function is similar to the one in GemmaInstance, except that all the conversation
    // items are in the "user" role.
    fn generate_contents(&self, conversation: &Conversation) -> Vec<RequestContent> {
        // 1. Build the special moderator prompt
        let mut contents = vec![RequestContent {
            role: "user".to_string(),
            parts: vec![Part {
                text: self.build_prompt(),
            }],
        }];

        // 2. Include the conversation history
        for item in conversation.values() {
            // put all ConvoItems in the "user" role
            let role = "user";

            // Prepend the speaker's name
            let text = format_message(&item.author, &item.message);

            // Attach message body
            contents.push(RequestContent {
                role: role.to_owned(),
                parts: vec![Part { text }],
            });
        }

        contents
    }
}

/********************* API types ************************************* */
// The actual request contents to send to the Gemini API
#[derive(Debug, Serialize)]
struct GemmaRequest {
    contents: Vec<RequestContent>,
    safety_settings: Vec<SafetySetting>,
    generation_config: GenerationConfig,
}

// Safety Settings of the response content as defined by Gemini API
#[derive(Debug, Serialize)]
struct SafetySetting {
    category: String,
    threshold: String,
}

// Config settings, of the responsecontent as defined by Gemini API
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
