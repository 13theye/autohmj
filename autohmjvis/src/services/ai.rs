// src/services/ai.rs
//
// Provider-agnostic AI service layer.
// Handles multiple AI personalities, the moderator pattern, and conversation formatting.

use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc};

use crate::{
    events::HMJEventBus,
    models::{Conversation, ConvoItem},
    settings::AIConfig,
};

// ===== Provider trait =====

/// Provider-agnostic message representation passed to AIProvider.
#[derive(Clone, Debug)]
pub struct AIMessage {
    pub role: AIRole,
    pub content: String,
}

/// Role of a message in the conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum AIRole {
    User,
    Model,
}

/// Interface between the AI service layer and a specific API provider.
pub trait AIProvider: Send + Sync + 'static {
    fn spawn_request(
        &self,
        messages: Vec<AIMessage>,
        tx: mpsc::Sender<Option<String>>,
        rthandle: &tokio::runtime::Handle,
        shutdown_rx: broadcast::Receiver<()>,
    );
}

// ===== Events =====

/// Events emitted by AIService to notify subscribers of responses.
#[derive(Clone, Debug)]
pub enum AIEvent {
    AIReceived(AIResponse),   // A response from an AI instance has been received.
    ModeratorChooses(String), // The ID of the persona who should speak next.
    AIRequested(String),      // A request has been sent to an AI instance (ID).
}

// ===== Service =====

pub struct AIService {
    pub instances: HashMap<AIPersona, AIInstance>,
    pub moderator: AIModerator,

    system_prompt: String,
    rthandle: tokio::runtime::Handle,

    provider: Box<dyn AIProvider>,

    event_tx: broadcast::Sender<AIEvent>,
    shutdown_tx: broadcast::Sender<()>,
}

impl AIService {
    pub fn new(
        config: &AIConfig,
        provider: Box<dyn AIProvider>,
        events: &HMJEventBus,
        rthandle: tokio::runtime::Handle,
    ) -> Self {
        let system_prompt = config.system.prompt.to_owned();
        let (shutdown_tx, _) = broadcast::channel(1);
        let (instances, moderator_instance) = make_instances_from_config(config);
        let moderator = AIModerator::from_ai_instance(moderator_instance);
        let event_tx = events.ai.clone();

        Self {
            instances,
            moderator,
            system_prompt,
            rthandle,
            provider,
            event_tx,
            shutdown_tx,
        }
    }

    pub fn update(&mut self) {
        self.receive_all();
    }

    /// Send a request to an AI instance.
    ///
    /// If the persona is AIPersona::Moderator, the conversation history is sent to the moderator
    /// instance to decide who should speak next.
    ///
    /// If the persona is a regular AI instance, the new message is sent to the instance.
    pub fn send(
        &mut self,
        ai_persona: AIPersona,
        new_item: Option<&ConvoItem>,
        conversation: &Conversation,
    ) -> Result<(), String> {
        let ai_instance: &AIInstance = if ai_persona == AIPersona::Moderator {
            &self.moderator.instance
        } else {
            let Some(instance) = self.instances.get(&ai_persona) else {
                return Err(format!(
                    "No AI instance found for persona: {:?}",
                    ai_persona
                ));
            };
            instance
        };

        // Format and generate provider-agnostic messages
        let messages = if ai_persona == AIPersona::Moderator {
            self.moderator.generate_contents(conversation)
        } else {
            ai_instance.generate_contents(new_item, conversation, &self.system_prompt)
        };

        let tx = ai_instance.tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        // Emit notification that an AI request has been sent
        let _ = self
            .event_tx
            .send(AIEvent::AIRequested(ai_instance.id.to_owned()));

        self.provider
            .spawn_request(messages, tx, &self.rthandle, shutdown_rx);

        Ok(())
    }

    /// Collect AI responses and broadcast them to the EventBus.
    ///
    /// IMPORTANT: currently, the moderator assumes that there is only one human in the conversation.
    pub fn receive_all(&mut self) {
        // Receive from moderator
        match self.moderator.instance.rx.try_recv() {
            Ok(Some(message)) => {
                if message.contains("Human") {
                    let _ = self
                        .event_tx
                        .send(AIEvent::ModeratorChooses("Human".to_string()));
                } else {
                    self.instances
                        .values()
                        .filter(|instance| message.contains(&instance.id))
                        .for_each(|instance| {
                            let _ = self
                                .event_tx
                                .send(AIEvent::ModeratorChooses(instance.id.clone()));
                        });
                }
            }
            Ok(None) => {
                println!("Received empty response from moderator");
                let _ = self
                    .event_tx
                    .send(AIEvent::ModeratorChooses("No Response".to_string()));
            }
            Err(_) => {}
        }

        // Collect responses from all AI instances
        for instance in self.instances.values_mut() {
            match instance.rx.try_recv() {
                Ok(Some(message)) => {
                    let _ = self.event_tx.send(AIEvent::AIReceived(AIResponse {
                        author: instance.id.to_owned(),
                        message,
                    }));
                }
                Ok(None) => {
                    println!("Received empty response from AI task: {}", instance.id);
                }
                Err(_) => {}
            }
        }
    }

    pub fn add(&mut self, persona: AIPersona, instance: AIInstance) {
        self.instances.insert(persona, instance);
    }

    pub fn shutdown(&mut self) {
        println!("...Shutting down AIService...");
        let _ = self.shutdown_tx.send(());
    }
}

impl Drop for AIService {
    fn drop(&mut self) {
        println!("...AIService being dropped");
        self.shutdown();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// ===== Instance =====

/// A single AI persona instance — holds personality and channels.
#[derive(Debug)]
pub struct AIInstance {
    pub id: String,
    personality: Personality,
    pub tx: mpsc::Sender<Option<String>>,
    pub rx: mpsc::Receiver<Option<String>>,
}

impl AIInstance {
    pub fn new(id: &str, personality: Personality) -> Self {
        let (tx, rx) = mpsc::channel(16);
        Self {
            id: id.to_owned(),
            personality,
            tx,
            rx,
        }
    }

    pub fn personality(&self) -> &str {
        &self.personality.prompt
    }

    fn build_prompt(&self, system_prompt: &str) -> String {
        system_prompt.to_owned() + " Personality: " + self.personality()
    }

    /// Generate provider-agnostic messages for this instance.
    pub fn generate_contents(
        &self,
        new_item: Option<&ConvoItem>,
        conversation: &Conversation,
        system_prompt: &str,
    ) -> Vec<AIMessage> {
        let mut messages = vec![
            // Personality/system prompt as first user message
            AIMessage {
                role: AIRole::User,
                content: self.build_prompt(system_prompt),
            },
        ];

        // Add conversation history
        for item in conversation.values() {
            let role = if item.author == self.id {
                AIRole::Model
            } else {
                AIRole::User
            };

            let content = if role == AIRole::User {
                format_message(&item.author, &item.message)
            } else {
                item.message.to_owned()
            };

            messages.push(AIMessage { role, content });
        }

        // Append new_item if provided
        if let Some(new_item) = new_item {
            messages.push(AIMessage {
                role: AIRole::User,
                content: format_message(&new_item.author, &new_item.message),
            });
        }

        messages
    }
}

// ===== Moderator =====

/// The AI Moderator receives the full conversation history and responds with the ID
/// of the persona who should speak next.
#[derive(Debug)]
pub struct AIModerator {
    pub instance: AIInstance,
}

impl AIModerator {
    pub fn new(id: &str, personality: Personality) -> Self {
        Self {
            instance: AIInstance::new(id, personality),
        }
    }

    pub fn from_ai_instance(instance: AIInstance) -> Self {
        Self { instance }
    }

    fn build_prompt(&self) -> String {
        self.instance.personality().to_owned()
    }

    /// Generate provider-agnostic messages for the moderator.
    /// All conversation items are placed in the User role.
    pub fn generate_contents(&self, conversation: &Conversation) -> Vec<AIMessage> {
        let mut messages = vec![
            // Moderator prompt as first user message
            AIMessage {
                role: AIRole::User,
                content: self.build_prompt(),
            },
        ];

        for item in conversation.values() {
            messages.push(AIMessage {
                role: AIRole::User,
                content: format_message(&item.author, &item.message),
            });
        }

        messages
    }
}

// ===== Helper types =====

/// A wrapper for a prompt defining an AI personality.
#[derive(Debug)]
pub struct Personality {
    pub prompt: String,
}

/// A parsed response from an AI persona.
#[derive(Clone, Debug)]
pub struct AIResponse {
    pub author: String,
    pub message: String,
}

/// The different AI personas.
#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
pub enum AIPersona {
    AI1,
    AI2,
    Moderator,
}

// ===== Helper functions =====

fn format_message(author: &str, message: &str) -> String {
    format!("{}: {}", author, message)
}

// Creates two AI instances and a moderator from the config file.
pub fn make_instances_from_config(
    config: &AIConfig,
) -> (HashMap<AIPersona, AIInstance>, AIInstance) {
    let mut personas = HashMap::new();

    let ai1 = AIInstance::new(
        &config.persona_1.id,
        Personality {
            prompt: config.persona_1.prompt.to_owned(),
        },
    );
    println!("AI1 ID: {:?}", ai1.id);

    let ai2 = AIInstance::new(
        &config.persona_2.id,
        Personality {
            prompt: config.persona_2.prompt.to_owned(),
        },
    );
    println!("AI2 ID: {:?}", ai2.id);

    personas.insert(AIPersona::AI1, ai1);
    personas.insert(AIPersona::AI2, ai2);

    let moderator = AIInstance::new(
        &config.moderator.id,
        Personality {
            prompt: config.moderator.prompt.to_owned(),
        },
    );
    println!("Moderator ID: {:?}", moderator.id);

    (personas, moderator)
}
