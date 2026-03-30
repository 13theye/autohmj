// src/events/bus.rs
//
// All event channels are defined here.

pub use crate::{
    content::ContentEvent,
    server::ServerEvent,
    services::{AIEvent, ConvoEvent, TranslationEvent},
    views::TypingAnimationEvent,
};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct HMJEventBus {
    pub animation: broadcast::Sender<TypingAnimationEvent>,
    pub convo: broadcast::Sender<ConvoEvent>,
    pub ai: broadcast::Sender<AIEvent>,
    pub server: broadcast::Sender<ServerEvent>,
    pub translation: broadcast::Sender<TranslationEvent>,
    pub content: broadcast::Sender<ContentEvent>,
}

impl Default for HMJEventBus {
    fn default() -> Self {
        let (animation_tx, _) = broadcast::channel(16);
        let (convo_tx, _) = broadcast::channel(16);
        let (ai_tx, _) = broadcast::channel(16);
        let (server_tx, _) = broadcast::channel(16);
        let (translation_tx, _) = broadcast::channel(16);
        let (content_tx, _) = broadcast::channel(16);

        Self {
            ai: ai_tx,
            animation: animation_tx,
            convo: convo_tx,
            server: server_tx,
            translation: translation_tx,
            content: content_tx,
        }
    }
}
