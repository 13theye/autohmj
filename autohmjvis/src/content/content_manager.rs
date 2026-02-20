//! src/content/content_manager.rs
//!
//! Content manager parses the content of a conversation and determines what to do
//! with the "latest" message in the Conversation.
//!

use crate::{
    content::ContentEvent,
    events::HMJEventBus,
    models::{ConvoItem, LiveInputRegistry},
    services::{ConvoEvent, GemmaEvent, TranslationEvent},
};

use tokio::sync::{broadcast, watch};

/// A tuple of (key, ConvoItem)
pub type KeyedConvoItem = (usize, ConvoItem);

pub struct ContentManager {
    pub id: String,

    pub latest: Option<KeyedConvoItem>, // the latest (key, message)
    live_input_rx: watch::Receiver<String>, // watch channel for this author's live input

    // event tx
    content_tx: broadcast::Sender<ContentEvent>,

    // event rx
    convo_rx: broadcast::Receiver<ConvoEvent>,
    translation_rx: broadcast::Receiver<TranslationEvent>,
    gemma_rx: broadcast::Receiver<GemmaEvent>,
}

impl ContentManager {
    pub fn new(
        id: &str,
        event_bus: &HMJEventBus,
        live_input_registry: &mut LiveInputRegistry,
    ) -> Self {
        // Subscribe to events
        let convo_rx = event_bus.convo.subscribe();
        let translation_rx = event_bus.translation.subscribe();
        let gemma_rx = event_bus.gemma.subscribe();

        // Get the Sender for this
        let content_tx = event_bus.content.clone();

        // Subscribe to this author's live input channel
        let live_input_rx = live_input_registry.subscribe(id);

        Self {
            id: id.to_owned(),
            latest: None,
            live_input_rx,
            convo_rx,
            translation_rx,
            gemma_rx,
            content_tx,
        }
    }

    /// Run once per app cycle. Reads the live input from the human user, updating
    pub fn update(&mut self) {
        self.read_live_input();
        self.process_events();
    }

    // Read the live input from the human user. Compare the input string to the previous
    // state of the input string.
    fn read_live_input(&mut self) {
        // Check if the live input has changed
        if !self.live_input_rx.has_changed().unwrap_or(false) {
            return;
        }

        // Get the current value and mark as seen
        let live_msg = self.live_input_rx.borrow_and_update().clone();

        // Don't do anything if there's no input and there's a previous message displayed.
        // This means live input is fresh ( no new live input since human last hit Enter)
        // This ensures that the previous latest message is not immediately overwritten.
        if live_msg.is_empty() && self.latest.is_some() {
            return;
        }

        // If there's no live input and latest is already empty, then we know that
        // we can clear the latest message because the user as reached this state by clearing
        // the live input.
        if live_msg.is_empty() && self.latest.is_none() {
            self.update_latest(None);
            return;
        }

        // Clear the latest message when the the live input is beginning to fill.
        if !live_msg.is_empty() && self.latest.is_some() {
            self.update_latest(None);
        }

        // Emit the live input event (only when changed)
        let _ = self.content_tx.send(ContentEvent::UpdatedLiveInput {
            source_id: self.id.to_owned(),
            content: live_msg.clone(),
        });
    }

    /// Listen for events from the ConversationManager.
    /// - if the author of the new message is the same as this grid's author, update the latest message
    ///   if it is different from the current latest message
    /// - if a translation has been received, update the latest message with the translation
    fn process_events(&mut self) {
        while let Ok(event) = self.convo_rx.try_recv() {
            // Update latest message if author is same as this grid's author
            if let ConvoEvent::ItemAdded(new_key, new_convo_item) = event {
                // Fire a content update event
                if new_convo_item.author == self.id {
                    // Skip if message is the same as latest
                    if self
                        .latest
                        .as_ref()
                        .is_some_and(|latest| new_key == latest.0)
                    {
                        continue;
                    }

                    // Update latest message
                    self.update_latest(Some((new_key, new_convo_item)));
                }
            }
        }

        // Update the translation
        while let Ok(event) = self.translation_rx.try_recv() {
            if let TranslationEvent::ItemTranslated {
                key,
                translation,
                is_2nd_translation,
            } = event
            {
                if let Some((latest_key, latest_entry)) = &self.latest {
                    if key == *latest_key {
                        let mut new_entry = latest_entry.to_owned();
                        if is_2nd_translation {
                            new_entry.translation2 = translation;
                        } else {
                            new_entry.translation = translation;
                        }
                        self.update_translation(Some((*latest_key, new_entry)));
                    }
                }
            }
        }

        while let Ok(event) = self.gemma_rx.try_recv() {
            if let GemmaEvent::GemmaRequested(id) = event {
                if id == self.id {
                    let _ = self
                        .content_tx
                        .send(ContentEvent::GemmaRequested(self.id.to_owned()));
                }
            }
        }
    }

    /// Update the latest message buffer and send corresponding event
    fn update_latest(&mut self, latest: Option<KeyedConvoItem>) {
        self.latest = latest;
        let _ = self.content_tx.send(ContentEvent::UpdatedLatest {
            source_id: self.id.to_owned(),
            convo_item: self.latest.to_owned(),
        });
    }

    fn update_translation(&mut self, latest: Option<KeyedConvoItem>) {
        self.latest = latest;
        let _ = self.content_tx.send(ContentEvent::UpdatedTranslation {
            source_id: self.id.to_owned(),
            convo_item: self.latest.to_owned(),
        });
    }
}
