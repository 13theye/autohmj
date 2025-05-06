// src/models/convo.rs
//
// Input history and translations

// Re-export Conversation Types
pub use autohmjcommon::{Conversation, ConvoItem, ConvoWrapper, HMJMessage};

// Maximum number of entries in the conversation history
const MAX_HISTORY: usize = 100;

// The ConversationManager maintains the convo history between client and AIs.
// Each ConvoItem contains the input from the client and its translation.
// ConversationManager fires off tasks to fetch translations for each entry once received.

#[derive(Default)]
pub struct ConversationManager {
    pub entries: Conversation,
    pub next_history_idx: usize,
}

impl ConversationManager {
    // Run once per cycle
    pub fn update(&mut self) {
        self.cleanup();
    }

    /************************* Input History management *****************************/

    // Add a new ConvoItem to the Conversation, returns the key that was issued
    pub fn add(&mut self, item: &ConvoItem) -> usize {
        let key = self.next_history_idx;
        self.entries.insert(key, item.clone());
        self.next_history_idx += 1;

        key
    }

    pub fn add_translation(&mut self, key: usize, translation: Option<String>) {
        if let Some(item) = self.entries.get_mut(&key) {
            item.translation = translation;
        }
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

    pub fn cleanup(&mut self) {
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
