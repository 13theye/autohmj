// src/services/translate.rs
//
// A Translation service that uses the external DeepLX crate to access DeepLX translation
//
//
use deeplx::{Config, DeepLX};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::events::{ConvoEvent, EventBus};

// Event interface
#[derive(Clone, Debug)]
pub enum TranslationEvent {
    ItemTranslated(usize, Option<String>), // key, translation
    DefaultEvent,
}

// Controller module for translations
pub struct TranslationService {
    translate: Arc<Translate>, // Translation API entry point
    pub translation_type: TranslationType,

    // Asynchronous translation
    pub runtime: Option<tokio::runtime::Runtime>,
    translation_tx: mpsc::Sender<(usize, Option<String>)>,
    translation_rx: mpsc::Receiver<(usize, Option<String>)>,

    // Events channel
    event_tx: broadcast::Sender<TranslationEvent>,
    history_rx: broadcast::Receiver<ConvoEvent>, // subscribe to HistoryEvents

    // Shutdown signal
    shutdown_tx: broadcast::Sender<()>, // Sender for shutdown signal
}

impl TranslationService {
    pub fn new(events: Arc<EventBus>) -> Self {
        // Set up eventbus send
        let event_tx = events.translation.clone();

        // Subscribe to other events
        let history_rx = events.convo.subscribe();

        // Set up async channel
        let (translation_tx, translation_rx) = mpsc::channel::<(usize, Option<String>)>(16);

        // Set up tokio translation runtime
        let translation_runtime =
            tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for translation");

        // Set up shutdown listener
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            translate: Arc::new(Translate::default()),
            translation_type: TranslationType::ToEnglish,
            runtime: Some(translation_runtime),
            translation_tx,
            translation_rx,
            event_tx,
            history_rx,
            shutdown_tx,
        }
    }

    pub fn update(&mut self) {
        self.process_events();
        self.receive_translations();
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.history_rx.try_recv() {
            if let ConvoEvent::ItemAdded(key, item) = event {
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

    fn request_translation(
        &mut self,
        key: usize,
        msg: String,
        translate: Arc<Translate>,
        translation_type: TranslationType,
    ) {
        if let Some(runtime) = &self.runtime {
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

    // Receive completed translations, update convo
    fn receive_translations(&mut self) {
        while let Ok((key, translation)) = self.translation_rx.try_recv() {
            let _ = self
                .event_tx
                .send(TranslationEvent::ItemTranslated(key, translation));
        }
    }

    fn shutdown(&mut self) {
        println!("...Shutting down TranslationService...");

        // Signal all tasks to terminate
        let _ = self.shutdown_tx.send(());

        // Take ownership of the runtime
        if let Some(runtime) = self.runtime.take() {
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

impl Drop for TranslationService {
    fn drop(&mut self) {
        println!("...TranslationService being dropped");
        self.shutdown();
        // Wait briefly
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TranslationType {
    ToEnglish,
    ToFrench,
    ToKorean,
}

#[derive(Clone)]
pub struct Translate {
    pub translator: DeepLX,
}

impl Default for Translate {
    fn default() -> Self {
        let deeplx_trans = DeepLX::new(Config {
            ..Default::default()
        });
        Self {
            translator: deeplx_trans,
        }
    }
}

impl Translate {
    async fn get_translation(
        &self,
        input: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Option<String> {
        let result = self
            .translator
            .translate(source_lang, target_lang, input, None, None)
            .await;

        match result {
            Ok(res) => Some(res.data.trim_end().to_owned()),
            Err(e) => {
                eprintln!("Error in Translate: {}", e);
                None
            }
        }
    }

    pub async fn to_english(&self, input: &str) -> Option<String> {
        self.get_translation(input, "auto", "en").await
    }

    pub async fn to_korean(&self, input: &str) -> Option<String> {
        self.get_translation(input, "auto", "ko").await
    }

    pub async fn to_french(&self, input: &str) -> Option<String> {
        self.get_translation(input, "auto", "fr").await
    }
}
