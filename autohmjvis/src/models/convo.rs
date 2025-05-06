// src/models/convo.rs
//
// Input history and translations

use crate::events::{EventBus, ServerEvent, TranslationEvent};
use std::sync::Arc;
use tokio::sync::broadcast;

// Re-export Conversation Types
pub use autohmjcommon::{Conversation, ConvoItem, ConvoWrapper, HMJMessage};

// Maximum number of entries in the conversation history
const MAX_HISTORY: usize = 100;

// Events interface
#[derive(Clone, Debug)]
pub enum ConvoEvent {
    ItemAdded(usize, ConvoItem),      // key, HistoryItem
    BroadcastConvoCommand(String),    // Request to broadcast message
    SendConvoCommand(String, String), // Request to send message (client_id, message)
}

// The ConversationService maintains the convo history between client and AIs.
// Each ConvoItem contains the input from the client and its translation.
// ConversationService fires off tasks to fetch translations for each entry once received.
pub struct ConversationService {
    pub entries: Conversation,
    pub next_history_idx: usize,

    // Events
    event_tx: broadcast::Sender<ConvoEvent>,
    translation_rx: broadcast::Receiver<TranslationEvent>, // subscribe to TranslationEvents
    server_rx: broadcast::Receiver<ServerEvent>,           // subscribe to NetworkEvents
}

impl ConversationService {
    pub fn new(events: Arc<EventBus>) -> Self {
        // Set up eventbus send
        let translation_rx = events.translation.subscribe();

        // Subscribe to other events
        let event_tx = events.convo.clone();
        let server_rx = events.server.subscribe();

        Self {
            entries: Conversation::default(),
            next_history_idx: 0,

            event_tx,
            translation_rx,
            server_rx,
        }
    }

    // Run once per cycle to manage the Convo queue and send off pending translation requests
    pub fn update(&mut self) {
        self.cleanup();
        self.process_events();
    }

    /************************* Handle Events *****************************/

    fn process_events(&mut self) {
        // Receive messages from TranslationService
        while let Ok(event) = self.translation_rx.try_recv() {
            if let TranslationEvent::ItemTranslated(key, translation) = event {
                if let Some(item) = self.entries.get_mut(&key) {
                    item.translation = translation;
                    let _ = self
                        .event_tx
                        .send(ConvoEvent::BroadcastConvoCommand(self.serialize()));
                }
            }
        }

        // Receive messages from WebSockets server
        while let Ok(event) = self.server_rx.try_recv() {
            match event {
                // Send serialized conversation to WebSocket server for broadcast
                ServerEvent::RequestConversation => {
                    let _ = self
                        .event_tx
                        .send(ConvoEvent::BroadcastConvoCommand(self.serialize()));
                }

                // Send serialized Conversation to WebSocket server to send to a specific client
                ServerEvent::RequestConversationFor(client_id) => {
                    let _ = self
                        .event_tx
                        .send(ConvoEvent::SendConvoCommand(client_id, self.serialize()));
                }
            }
        }
    }

    /************************* Input History management *****************************/

    // Create and return a HistoryItem
    pub fn new_item(author: &str, message: &str) -> ConvoItem {
        ConvoItem::new(author, message)
    }

    // Add a new ConvoItem to the Conversation
    pub fn add(&mut self, item: ConvoItem) {
        let key = self.next_history_idx;
        self.entries.insert(key, item.clone());
        self.next_history_idx += 1;

        // Notify event subscribers
        let _ = self.event_tx.send(ConvoEvent::ItemAdded(key, item));
        let _ = self
            .event_tx
            .send(ConvoEvent::BroadcastConvoCommand(self.serialize()));
    }

    // Get a reference to the most recent HistoryItem from a given author
    pub fn get_latest_by_author(&self, author: &str) -> Option<&ConvoItem> {
        self.entries
            .iter()
            .rev()
            .map(|(_, item)| item)
            .find(|&item| item.author == *author)
    }

    pub fn remove(&mut self, key: &usize) {
        self.entries.remove(key);
    }

    fn cleanup(&mut self) {
        while self.entries.len() > MAX_HISTORY {
            if let Some(smallest_key) = self.entries.keys().next().copied() {
                self.remove(&smallest_key);
            }
        }
    }

    /************************* Output / Networking *****************************/

    // Convert Conversation to a serialized string
    pub fn serialize(&self) -> String {
        let result = serde_json::to_string(&ConvoWrapper(self.entries.clone()));
        match result {
            Ok(string) => string,
            Err(e) => {
                eprintln!("Error serializing conversation: {}", e);
                String::new()
            }
        }
    }
}
