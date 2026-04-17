// src/services/ai.rs
//
// Provider-agnostic AI service layer.
// Handles multiple AI personalities, the moderator pattern, and conversation formatting.

use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc};

use crate::{
    events::HMJEventBus,
    models::{Conversation, ConvoItem},
    settings::PersonaConfig,
};

// ===== Provider context types =====

/// Raw semantic context passed to a provider.
/// Providers receive this and perform all message formatting themselves.
#[derive(Clone, Debug)]
pub enum AIContext {
    /// A regular persona turn.
    Persona(PersonaContext),
    /// The moderator turn — all history flattened, no system prompt.
    Moderator(ModeratorContext),
}

/// Payload returned from a provider via the instance channel.
/// `response_id` is populated by providers that support persistent state
/// (e.g. OpenAI Responses API); others set it to `None`.
#[derive(Debug)]
pub struct ProviderOutput {
    pub message: String,
    pub response_id: Option<String>,
}

/// Everything a provider needs to format a persona request.
#[derive(Clone, Debug)]
pub struct PersonaContext {
    /// System prompt and personality concatenated: `system_prompt + " Personality: " + personality`
    pub instructions: String,
    /// The ID of the persona making this request (used to attribute model vs. user role).
    pub persona_id: String,
    /// Ordered conversation history at the time of the request.
    /// When `previous_response_id` is set this contains only the messages added
    /// since the last request to this persona; otherwise it contains the full history.
    pub history: Vec<ContextMessage>,
    /// Optional new message to append (not yet in `history`).
    pub new_item: Option<ContextMessage>,
    /// ID of this persona's previous response. When `Some`, the provider should
    /// send only the incremental history and link the request to the prior turn.
    pub previous_response_id: Option<String>,
}

/// Everything a provider needs to format a moderator request.
#[derive(Clone, Debug)]
pub struct ModeratorContext {
    /// The moderator's personality prompt.
    pub instructions: String,
    /// Ordered conversation history at the time of the request.
    /// When `previous_response_id` is set this contains only the messages added
    /// since the last moderator request; otherwise it contains the full history.
    pub history: Vec<ContextMessage>,
    /// ID of the moderator's previous response. Same semantics as `PersonaContext`.
    pub previous_response_id: Option<String>,
}

/// A single conversation entry — author + text. Providers assign roles themselves.
#[derive(Clone, Debug)]
pub struct ContextMessage {
    pub author: String,
    pub content: String,
}

// ===== Provider trait =====

/// Interface between the AI service layer and a specific API provider.
pub trait AIProvider: Send + Sync + 'static {
    fn spawn_request(
        &self,
        context: AIContext,
        tx: mpsc::Sender<Option<ProviderOutput>>,
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
    AIFailed(String),         // A request to an AI instance failed (payload: persona ID).
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
        persona_1: &PersonaConfig,
        persona_2: &PersonaConfig,
        moderator: &PersonaConfig,
        system_prompt: String,
        provider: Box<dyn AIProvider>,
        events: &HMJEventBus,
        rthandle: tokio::runtime::Handle,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let (instances, moderator_instance) =
            make_instances_from_config(persona_1, persona_2, moderator);
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
        let full_history: Vec<ContextMessage> = conversation
            .values()
            .map(|item| ContextMessage {
                author: item.author.clone(),
                content: item.message.clone(),
            })
            .collect();

        let (context, tx, instance_id) = if ai_persona == AIPersona::Moderator {
            let prev_id = self.moderator.instance.last_response_id.clone();
            let history = incremental_history(&full_history, &self.moderator.instance.id, &prev_id);
            let ctx = AIContext::Moderator(ModeratorContext {
                instructions: self.moderator.instance.personality().to_owned(),
                history,
                previous_response_id: prev_id,
            });
            let tx = self.moderator.instance.tx.clone();
            let id = self.moderator.instance.id.clone();
            (ctx, tx, id)
        } else {
            let Some(instance) = self.instances.get_mut(&ai_persona) else {
                return Err(format!(
                    "No AI instance found for persona: {:?}",
                    ai_persona
                ));
            };
            if instance.pending {
                println!(
                    "AIService::send: request already pending for {}, skipping",
                    instance.id
                );
                return Ok(());
            }
            instance.pending = true;
            let prev_id = instance.last_response_id.clone();
            let history = incremental_history(&full_history, &instance.id, &prev_id);
            let instructions = format!(
                "{} Personality: {}",
                self.system_prompt,
                instance.personality()
            );
            let new_item = new_item.map(|i| ContextMessage {
                author: i.author.clone(),
                content: i.message.clone(),
            });
            let ctx = AIContext::Persona(PersonaContext {
                instructions,
                persona_id: instance.id.clone(),
                history,
                new_item,
                previous_response_id: prev_id,
            });
            let tx = instance.tx.clone();
            let id = instance.id.clone();
            (ctx, tx, id)
        };

        let shutdown_rx = self.shutdown_tx.subscribe();
        let _ = self.event_tx.send(AIEvent::AIRequested(instance_id));
        self.provider
            .spawn_request(context, tx, &self.rthandle, shutdown_rx);

        Ok(())
    }

    /// Collect AI responses and broadcast them to the EventBus.
    ///
    /// IMPORTANT: currently, the moderator assumes that there is only one human in the conversation.
    pub fn receive_all(&mut self) {
        // Receive from moderator
        match self.moderator.instance.rx.try_recv() {
            Ok(Some(output)) => {
                self.moderator.instance.last_response_id = output.response_id;
                let message = output.message;
                println!("Received response from moderator: {:#?}", message);
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
                Ok(Some(output)) => {
                    instance.pending = false;
                    instance.last_response_id = output.response_id;
                    let message = output.message;
                    println!("Received response from Ai instance: {:#?}", message);

                    let _ = self.event_tx.send(AIEvent::AIReceived(AIResponse {
                        author: instance.id.to_owned(),
                        message,
                    }));
                }
                Ok(None) => {
                    instance.pending = false;
                    println!("Received empty response from AI task: {}", instance.id);
                    let _ = self.event_tx.send(AIEvent::AIFailed(instance.id.to_owned()));
                }
                Err(_) => {}
            }
        }
    }

    /// Clear all stored response IDs.
    /// Call this when the conversation is reset so the next request sends
    /// full history again rather than an incremental slice.
    pub fn reset_response_ids(&mut self) {
        for instance in self.instances.values_mut() {
            instance.last_response_id = None;
            instance.pending = false;
        }
        self.moderator.instance.last_response_id = None;
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

/// A single AI persona instance — holds personality, channels, and stateful request tracking.
#[derive(Debug)]
pub struct AIInstance {
    pub id: String,
    personality: Personality,
    pub tx: mpsc::Sender<Option<ProviderOutput>>,
    pub rx: mpsc::Receiver<Option<ProviderOutput>>,
    /// The `id` from the most recent response. Used as `previous_response_id`
    /// on the next request to enable stateful multi-turn conversations.
    pub last_response_id: Option<String>,
    /// True while a request is in-flight. Prevents duplicate requests to the same persona.
    pub pending: bool,
}

impl AIInstance {
    pub fn new(id: &str, personality: Personality) -> Self {
        let (tx, rx) = mpsc::channel(16);
        Self {
            id: id.to_owned(),
            personality,
            tx,
            rx,
            last_response_id: None,
            pending: false,
        }
    }

    pub fn personality(&self) -> &str {
        &self.personality.prompt
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

// Creates two AI instances and a moderator from persona configs.
pub fn make_instances_from_config(
    persona_1: &PersonaConfig,
    persona_2: &PersonaConfig,
    moderator: &PersonaConfig,
) -> (HashMap<AIPersona, AIInstance>, AIInstance) {
    let mut personas = HashMap::new();

    let ai1 = AIInstance::new(
        &persona_1.id,
        Personality {
            prompt: persona_1.prompt.to_owned(),
        },
    );
    println!("AI1 ID: {:?}", ai1.id);

    let ai2 = AIInstance::new(
        &persona_2.id,
        Personality {
            prompt: persona_2.prompt.to_owned(),
        },
    );
    println!("AI2 ID: {:?}", ai2.id);

    personas.insert(AIPersona::AI1, ai1);
    personas.insert(AIPersona::AI2, ai2);

    let moderator_instance = AIInstance::new(
        &moderator.id,
        Personality {
            prompt: moderator.prompt.to_owned(),
        },
    );
    println!("Moderator ID: {:?}", moderator_instance.id);

    (personas, moderator_instance)
}

/// Returns the slice of `history` that the server does not already have.
///
/// When `previous_response_id` is `Some`, the server's stored context ends at
/// this persona's last response. We find that response's position and return
/// everything after it — the messages from other participants the server hasn't
/// seen. If no prior response is found or `previous_response_id` is `None`,
/// the full history is returned (first turn or post-reset).
fn incremental_history(
    history: &[ContextMessage],
    persona_id: &str,
    previous_response_id: &Option<String>,
) -> Vec<ContextMessage> {
    if previous_response_id.is_none() {
        return history.to_vec();
    }
    let start = history
        .iter()
        .rposition(|m| m.author == persona_id)
        .map(|i| i + 1)
        .unwrap_or(0);
    history[start..].to_vec()
}
