//! src/models/live_input.rs
//!
//! Registry for managing live input from WebSocket clients using tokio watch channels.
//! Each author (user/persona) has their own watch channel that observers can subscribe to.

use std::collections::HashMap;
use tokio::sync::watch;

/// Manages per-author watch channels for live input
pub struct LiveInputRegistry {
    channels: HashMap<String, watch::Sender<String>>,
}

impl LiveInputRegistry {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Get or create a watch receiver for a specific author
    pub fn subscribe(&mut self, author: &str) -> watch::Receiver<String> {
        let sender = self
            .channels
            .entry(author.to_owned())
            .or_insert_with(|| watch::channel(String::new()).0);

        sender.subscribe()
    }

    /// Update the live input for a specific author
    pub fn update(&mut self, author: &str, message: String) {
        let sender = self
            .channels
            .entry(author.to_owned())
            .or_insert_with(|| watch::channel(String::new()).0);

        // Only send if the value has changed (watch does this automatically)
        let _ = sender.send(message);
    }

    /// Clear the live input for a specific author (set to empty string)
    pub fn clear(&mut self, author: &str) {
        if let Some(sender) = self.channels.get(author) {
            let _ = sender.send(String::new());
        }
    }
}

impl Default for LiveInputRegistry {
    fn default() -> Self {
        Self::new()
    }
}
