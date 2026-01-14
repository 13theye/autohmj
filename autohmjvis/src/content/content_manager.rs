//! src/content/content_manager.rs
//!
//! Content manager parses the content of a conversation and determines what to do
//! with the "latest" message in the Conversation.
//!

use crate::{
    config::OscSendConfig,
    content::ContentEvent,
    events::HMJEventBus,
    models::ConvoItem,
    services::{ConvoEvent, Sequencer, TranslationEvent},
};

use tokio::sync::broadcast;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A tuple of (key, ConvoItem)
pub type KeyedConvoItem = (usize, ConvoItem);

pub struct ContentManager {
    pub id: String,

    pub latest: Option<KeyedConvoItem>, // the latest (key, message)
    connections: Arc<RwLock<HashMap<String, String>>>, // reference to model.connections

    // event tx
    pub content_tx: broadcast::Sender<ContentEvent>,

    // event rx
    convo_rx: broadcast::Receiver<ConvoEvent>,
    translation_rx: broadcast::Receiver<TranslationEvent>,

    // OSC Sequencer
    pub sequencer: Sequencer,
}

impl ContentManager {
    pub fn new(
        id: &str,
        event_bus: &HMJEventBus,
        connections: Arc<RwLock<HashMap<String, String>>>,
        osc_config: &OscSendConfig,
    ) -> Self {
        // Initialize OSC Sender
        let sequencer = Sequencer::new(id, osc_config);

        // Subscribe to events
        let convo_rx = event_bus.convo.subscribe();
        let translation_rx = event_bus.translation.subscribe();

        // Initialize a content channel
        let (content_tx, _) = broadcast::channel(16);

        Self {
            id: id.to_owned(),
            latest: None,
            connections,
            convo_rx,
            translation_rx,
            content_tx,
            sequencer,
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
        let connections_read = self.connections.read().unwrap_or_else(|poisoned| {
            println!("Warning: Connections RwLock is poisoned: {poisoned}");
            poisoned.into_inner()
        });

        let Some(human_msg) = connections_read.get(&self.id) else {
            return;
        };

        let human_msg = human_msg.clone();

        // Drop the lock
        drop(connections_read);

        // Don't do anything if there's no input and there's a previous message displayed.
        if human_msg.is_empty() && self.latest.is_some() {
            return;
        }

        // Clear the latest message if the the live input is filling
        if !human_msg.is_empty() && self.latest.is_some() {
            self.update_latest(None);
        }

        // Emit the live input event
        let _ = self.content_tx.send(ContentEvent::UpdatedLiveInput(
            self.id.to_owned(),
            human_msg.to_owned(),
        ));
    }

    // This function listens for ItemAdded messages from the ConversationService. If the
    // author of the message is this grid's author, it reads that message and generates
    // CharacterEntities from the the message.
    //
    // If the author is an AI, we know that there is only one way to handle the content:
    // run the "typing" animation and feed the currently visible characters to the Sequencer.
    //
    // If the author is human, there are two possibilities:
    // 1. As above
    // 2. Message is currently "in progress" -- so need need to handle the input via
    // ...the Connections HashMap in Main.

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
            if let TranslationEvent::ItemTranslated(key, translation) = event {
                if let Some((latest_key, latest_entry)) = &self.latest {
                    if key == *latest_key {
                        let mut new_entry = latest_entry.to_owned();
                        new_entry.translation = translation;
                        self.update_latest(Some((*latest_key, new_entry)));
                    }
                }
            }
        }
    }

    /// Update the latest message buffer and send corresponding event
    fn update_latest(&mut self, latest: Option<KeyedConvoItem>) {
        self.latest = latest;
        let _ = self
            .content_tx
            .send(ContentEvent::UpdatedLatest(self.latest.to_owned()));
    }
}
