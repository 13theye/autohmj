// src/events/bus.rs
//
// Event bus

pub use crate::{
    server::ServerEvent,
    services::{ConvoEvent, GemmaEvent, TranslationEvent},
    views::AnimationEvent,
};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct EventBus {
    pub animation: broadcast::Sender<AnimationEvent>,
    pub convo: broadcast::Sender<ConvoEvent>,
    pub gemma: broadcast::Sender<GemmaEvent>,
    pub server: broadcast::Sender<ServerEvent>,
    pub translation: broadcast::Sender<TranslationEvent>,
}

impl Default for EventBus {
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
        }
    }
}
