// src/services/convo.rs
//
// Interface for the ConversationManager data model

use crate::events::{EventBus, GemmaEvent, ServerEvent, TranslationEvent};
use crate::models::{Conversation, ConversationManager, ConvoItem};

use tokio::sync::broadcast;

// ConvoEvents are emitted by ConversationService to notify subscribers of changes to the
// ConversationManager data model. Subscribers generally expect a ConvoEvent as a response
// to a request for ConversationService to modify the Conversation data.
#[derive(Clone, Debug)]
pub enum ConvoEvent {
    ItemAdded(usize, ConvoItem),      // key, HistoryItem
    BroadcastConvoCommand(String),    // Request to broadcast message
    SendConvoCommand(String, String), // Request to send message (client_id, message)
}

// ConversationService is the interface for the ConversationManager data model.
// It is responsible for managing the ConversationManager data model and
// notifying subscribers of changes to the ConversationManager data model.
// It is also responsible for receiving events from the EventBus and
// triggering appropriate actions.
pub struct ConversationService {
    conversation: ConversationManager,

    // Events
    event_tx: broadcast::Sender<ConvoEvent>,
    translation_rx: broadcast::Receiver<TranslationEvent>, // subscribe to TranslationEvents
    server_rx: broadcast::Receiver<ServerEvent>,           // subscribe to Websocket ServerEvents
    gemma_rx: broadcast::Receiver<GemmaEvent>,             // subscribe to GemmaEvents
}

impl ConversationService {
    pub fn new(events: &EventBus) -> Self {
        // Set up eventbus send
        let translation_rx = events.translation.subscribe();

        // Subscribe to other events
        let event_tx = events.convo.clone();
        let server_rx = events.server.subscribe();
        let gemma_rx = events.gemma.subscribe();

        // Initialize ConversationManager
        let conversation = ConversationManager::default();

        Self {
            conversation,
            event_tx,
            translation_rx,
            server_rx,
            gemma_rx,
        }
    }

    // Run once per cycle
    pub fn update(&mut self) {
        self.conversation.update();
        self.process_events();
    }

    fn process_events(&mut self) {
        // Receive messages from TranslationService
        while let Ok(event) = self.translation_rx.try_recv() {
            if let TranslationEvent::ItemTranslated(key, translation) = event {
                // Add translation to conversation
                self.add_translation(key, translation);

                // Broadcast Conversation
                let _ = self.event_tx.send(ConvoEvent::BroadcastConvoCommand(
                    self.conversation.serialize(),
                ));
            }
        }

        // Receive messages from Gemma
        while let Ok(event) = self.gemma_rx.try_recv() {
            if let GemmaEvent::GemmaReceived(response) = event {
                // Add conversation item & notify subscribers
                let item = Self::new_item(&response.author, &response.message);
                self.add(item);
            }
        }

        // Receive messages from WebSockets server
        while let Ok(event) = self.server_rx.try_recv() {
            match event {
                // Send serialized conversation to WebSocket server for broadcast
                ServerEvent::NeedConversation => {
                    let _ = self.event_tx.send(ConvoEvent::BroadcastConvoCommand(
                        self.conversation.serialize(),
                    ));
                }

                // Send serialized Conversation to WebSocket server to send to a specific client
                ServerEvent::NeedConversationFor(client_id) => {
                    let _ = self.event_tx.send(ConvoEvent::SendConvoCommand(
                        client_id,
                        self.conversation.serialize(),
                    ));
                }
            }
        }
    }

    // Add a new ConvoItem to the Conversation
    pub fn add(&mut self, item: ConvoItem) {
        let key = self.conversation.add(&item);

        // Notify event subscribers
        let _ = self.event_tx.send(ConvoEvent::ItemAdded(key, item));
        let _ = self.event_tx.send(ConvoEvent::BroadcastConvoCommand(
            self.conversation.serialize(),
        ));
    }

    // Add a translation to a ConvoItem model entry via the ConversationManager
    fn add_translation(&mut self, key: usize, translation: Option<String>) {
        self.conversation.add_translation(key, translation);
    }

    // Get a reference to the Entries of a conversation
    pub fn entries(&self) -> &Conversation {
        &self.conversation.entries
    }

    // Get an author's latest entry in the conversation history.
    pub fn get_latest_by_author(&self, author: &str) -> Option<&ConvoItem> {
        self.conversation.get_latest_by_author(author)
    }

    // Create a new ConvoItem from an author and message.
    pub fn new_item(author: &str, message: &str) -> ConvoItem {
        ConvoItem::new(author, message)
    }

    // Clear the conversation history.
    pub fn reset(&mut self) {
        self.conversation.reset();
    }
}
