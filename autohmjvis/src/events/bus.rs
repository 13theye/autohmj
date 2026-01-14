// src/events/bus.rs
//
// All event channels are defined here.

pub use crate::{
    content::ContentEvent,
    server::ServerEvent,
    services::{ConvoEvent, GemmaEvent, TranslationEvent},
    views::AnimationEvent,
};
use tokio::sync::broadcast;

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct HMJEventBus {
    pub animation: broadcast::Sender<AnimationEvent>,
    pub convo: broadcast::Sender<ConvoEvent>,
    pub gemma: broadcast::Sender<GemmaEvent>,
    pub server: broadcast::Sender<ServerEvent>,
    pub translation: broadcast::Sender<TranslationEvent>,

    // Tx channels for ContentManagers, indexed by ContentManager.id
    pub content: HashMap<String, broadcast::Sender<ContentEvent>>,
}

impl Default for HMJEventBus {
    fn default() -> Self {
        let (animation_tx, _) = broadcast::channel(16);
        let (convo_tx, _) = broadcast::channel(16);
        let (gemma_tx, _) = broadcast::channel(16);
        let (server_tx, _) = broadcast::channel(16);
        let (translation_tx, _) = broadcast::channel(16);

        Self {
            gemma: gemma_tx,
            animation: animation_tx,
            convo: convo_tx,
            server: server_tx,
            translation: translation_tx,
            content: HashMap::new(),
        }
    }
}

impl HMJEventBus {
    /// Register a new content manager to the content events channel
    pub fn register_content_manager(&mut self, id: &str, tx: &broadcast::Sender<ContentEvent>) {
        self.content.insert(id.to_owned(), tx.clone());
    }
}
