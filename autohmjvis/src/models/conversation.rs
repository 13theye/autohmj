// src/models/conversation.rs
//
// The conversation data model

use std::collections::HashMap;

// Re-export Conversation Types
pub use autohmjcommon::{Conversation, ConvoItem, ConvoWrapper, HMJMessageWrapper};

// Maximum number of entries in the conversation history
const MAX_HISTORY: usize = 100;

// The ConversationManager maintains the convo history between client and AIs.
// Each ConvoItem contains the input from the client and its translation.
// ConversationManager fires off tasks to fetch translations for each entry once received.

#[derive(Default)]
pub struct ConversationManager {
    pub entries: Conversation,
    pub next_history_idx: usize,
    pub latest_by_author: HashMap<String, usize>,
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
        self.latest_by_author.insert(item.author.to_owned(), key);
        self.next_history_idx += 1;

        key
    }

    // Add a translation to a ConvoItem
    pub fn add_translation(
        &mut self,
        key: usize,
        translation: Option<String>,
        is_2nd_translation: bool,
    ) {
        if let Some(item) = self.entries.get_mut(&key) {
            if is_2nd_translation {
                item.translation2 = translation;
            } else {
                item.translation = translation;
            }
        }
    }

    // Get a reference to the most recent ConvoItem from a given author
    pub fn get_latest_by_author(&self, author: &str) -> Option<&ConvoItem> {
        if let Some(key) = self.latest_by_author.get(author) {
            self.entries.get(key)
        } else {
            None
        }
    }

    // Remove a ConvoItem from the Conversation
    pub fn remove(&mut self, key: &usize) {
        self.entries.remove(key);
    }

    // Clear all entries and reset the conversation
    pub fn reset(&mut self) {
        self.entries.clear();
        self.latest_by_author.clear();
        self.next_history_idx = 0;
    }

    // Remove oldest entries if the conversation history exceeds the maximum length
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
