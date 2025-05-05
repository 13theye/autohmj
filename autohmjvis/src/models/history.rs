// src/models/history.rs
//
// Input history and translations

use crate::services::{Translate, TranslationType};
use std::{collections::VecDeque, sync::Arc};
use tokio::sync::{broadcast, mpsc};

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

    // Translation
    translate: Arc<Translate>,          // Translation API entry point
    translation_queue: VecDeque<usize>, // Keys of items awaiting translation
    pub translation_type: TranslationType,

    // Asynchronous translation
    pub translation_runtime: Option<tokio::runtime::Runtime>,
    translation_tx: mpsc::Sender<(usize, Option<String>)>,
    translation_rx: mpsc::Receiver<(usize, Option<String>)>,

    // Flag that the history has been updated and needs to be broadcast to clients
    pub needs_broadcast: bool,

    // Shutdown signal
    shutdown_tx: broadcast::Sender<()>, // Sender for shutdown signal
}

impl Default for HistoryManager {
    fn default() -> Self {
        // Set up translation send/receive
        let (translation_tx, translation_rx) = mpsc::channel::<(usize, Option<String>)>(16);

        // Set up tokio translation runtime
        let translation_runtime =
            tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for translation");

        // Set up shutdown listener
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            entries: History::default(),
            next_history_idx: 0,

            translate: Arc::new(Translate::default()),
            translation_queue: VecDeque::new(),
            translation_type: TranslationType::ToEnglish,
            translation_runtime: Some(translation_runtime),

            translation_tx,
            translation_rx,

            needs_broadcast: false,

            shutdown_tx,
        }
    }
}

impl HistoryManager {
    // Create and return a HistoryItem
    pub fn new_item(author: &str, message: &str) -> HistoryItem {
        HistoryItem::new(author, message)
    }

    // Run once per cycle to manage the history queue and send off pending translation requests
    pub fn update(&mut self) {
        self.cleanup();
        self.process_translation_queue();
        self.receive_translations();
    }

    // Get a reference to the most recent HistoryItem from a given author
    pub fn get_latest_by_author(&self, author: &str) -> Option<&HistoryItem> {
        self.entries
            .iter()
            .rev()
            .map(|(_, item)| item)
            .find(|&item| item.author == *author)
    }
    /************************* Input History management *****************************/

    // Add a new HistoryItem to the History
    pub fn add(&mut self, item: HistoryItem) {
        let key = self.next_history_idx;
        self.entries.insert(key, item);
        self.next_history_idx += 1;
        self.queue_for_translation(key);
        self.needs_broadcast = true;
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

    /************************* Translations *****************************/

    // Add a key to the translation queue
    fn queue_for_translation(&mut self, key: usize) {
        self.translation_queue.push_back(key);
    }

    // Checks that a translation queue item is valid and sends it off to translation API
    fn process_translation_queue(&mut self) {
        while !self.translation_queue.is_empty() {
            if let Some(key) = self.translation_queue.pop_front() {
                if let Some(item) = self.entries.get(&key) {
                    if item.translation.is_none() && !item.message.trim().is_empty() {
                        self.request_translation(
                            key,
                            item.message.clone(),
                            self.translate.clone(),
                            self.translation_type.clone(),
                        );
                    }
                }
            }
        }
    }

    // Spawns an async task to request a translation via Translate API
    fn request_translation(
        &mut self,
        key: usize,
        msg: String,
        translate: Arc<Translate>,
        translation_type: TranslationType,
    ) {
        if let Some(runtime) = &self.translation_runtime {
            let tx = self.translation_tx.clone();
            let mut shutdown_rx = self.shutdown_tx.subscribe();
            runtime.spawn(async move {
                // Spawn shutdown future
                let shutdown = async {
                    let _ = shutdown_rx.recv().await;
                };

                let task = async {
                    let translation = match translation_type {
                        TranslationType::ToKorean => translate.to_korean(&msg).await,
                        TranslationType::ToEnglish => translate.to_english(&msg).await,
                        TranslationType::ToFrench => translate.to_french(&msg).await,
                    };
                    let _ = tx.send((key, translation)).await;
                };

                tokio::select! {
                    _ = shutdown => {
                        println!("--- Translation task received shutdown signal");
                    }
                    _ = task => {
                        println!("Translation task completed normally")
                    }
                }
            });
        }
    }

    // Receive completed translations, update history
    fn receive_translations(&mut self) {
        while let Ok((key, translation)) = self.translation_rx.try_recv() {
            if let Some(item) = self.entries.get_mut(&key) {
                item.translation = translation;
                self.needs_broadcast = true;
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

    /**************************** Shutdown **************************************/

    pub fn shutdown(&mut self) {
        println!("...Shutting down HistoryManager...");

        // Signal all tasks to terminate
        let _ = self.shutdown_tx.send(());

        // Take ownership of the runtime
        if let Some(runtime) = self.translation_runtime.take() {
            // Shut down runtime from a separate thread to avoid blocking
            std::thread::spawn(move || {
                println!(".....Shutting down Translation runtime in separate thread...");
                runtime.shutdown_timeout(std::time::Duration::from_secs(1));
            })
            .join()
            .ok();
            println!(".....Translation runtime shutdown successfully");
        }
    }
}

impl Drop for HistoryManager {
    fn drop(&mut self) {
        println!("...HistoryManager being dropped");
        self.shutdown();
        // Wait briefly
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
