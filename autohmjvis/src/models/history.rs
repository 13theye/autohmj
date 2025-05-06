// src/models/history.rs
//
// Input history and translations

use crate::events::HistoryEvent;
use tokio::sync::broadcast;

// Re-export History Types
pub use autohmjcommon::{HMJMessage, History, HistoryItem, HistoryWrapper};

// Maximum number of entries in the history
const MAX_HISTORY: usize = 100;

// The HistoryManager maintains the input history between client and AIs.
// Each HistoryItem contains the input from the client and its translation.
// HistoryManager fires off tasks to fetch translations for each entry once received.
pub struct HistoryManager {
    pub entries: History,
    pub next_history_idx: usize,

    // Events
    history_event_tx: broadcast::Sender<HistoryEvent>,
    history_event_rx: broadcast::Receiver<HistoryEvent>,
}

impl HistoryManager {
    pub fn new(history_tx: &broadcast::Sender<HistoryEvent>) -> Self {
        Self {
            entries: History::default(),
            next_history_idx: 0,

            history_event_tx: history_tx.clone(),
            history_event_rx: history_tx.subscribe(),
        }
    }

    // Run once per cycle to manage the history queue and send off pending translation requests
    pub fn update(&mut self) {
        self.cleanup();
        self.process_events();
    }

    /************************* Handle Events *****************************/

    fn process_events(&mut self) {
        while let Ok(event) = self.history_event_rx.try_recv() {
            if let HistoryEvent::ItemTranslated(key, translation) = event {
                if let Some(item) = self.entries.get_mut(&key) {
                    item.translation = translation;
                    let _ = self
                        .history_event_tx
                        .send(HistoryEvent::BroadcastHistory(self.serialize()));
                }
            }
        }
    }

    /************************* Input History management *****************************/

    // Create and return a HistoryItem
    pub fn new_item(author: &str, message: &str) -> HistoryItem {
        HistoryItem::new(author, message)
    }

    // Add a new HistoryItem to the History
    pub fn add(&mut self, item: HistoryItem) {
        let key = self.next_history_idx;
        self.entries.insert(key, item.clone());
        self.next_history_idx += 1;

        // Notify event subscribers
        let _ = self
            .history_event_tx
            .send(HistoryEvent::ItemAdded(key, item));
        let _ = self
            .history_event_tx
            .send(HistoryEvent::BroadcastHistory(self.serialize()));
    }

    // Get a reference to the most recent HistoryItem from a given author
    pub fn get_latest_by_author(&self, author: &str) -> Option<&HistoryItem> {
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

    // Convert History to a serialized string
    pub fn serialize(&self) -> String {
        let result = serde_json::to_string(&HistoryWrapper(self.entries.clone()));
        match result {
            Ok(string) => string,
            Err(e) => {
                eprintln!("Error serializing input history: {}", e);
                String::new()
            }
        }
    }
}
