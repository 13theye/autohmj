// src/services/translate.rs
//
// A Translation service that uses the external DeepLX crate to access DeepLX translation
//
//
use deeplx::{Config, DeepLX};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::events::{ConvoEvent, HMJEventBus};

// Event interface
#[derive(Clone, Debug)]
pub enum TranslationEvent {
    ItemTranslated {
        key: usize,
        translation: Option<String>,
        is_2nd_translation: bool,
    },
    DefaultEvent,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TranslationServiceId {
    First,
    Second,
}

// Controller module for translations
pub struct TranslationService {
    id: TranslationServiceId,
    translate: Arc<Translate>, // Translation API entry point
    pub translation_language: TranslationLanguage, // the destination language
    pub enabled: bool,         // whether translation is enabled

    // Asynchronous translation
    pub runtime: Option<tokio::runtime::Runtime>, // the runtime for the translation
    translation_tx: mpsc::Sender<(usize, Option<String>, TranslationServiceId)>, // the channel for sending translations
    translation_rx: mpsc::Receiver<(usize, Option<String>, TranslationServiceId)>, // the channel for receiving translations

    // Events channel
    event_tx: broadcast::Sender<TranslationEvent>, // the channel for sending translation events
    history_rx: broadcast::Receiver<ConvoEvent>,   // subscribe to HistoryEvents

    // Shutdown signal
    shutdown_tx: broadcast::Sender<()>, // Sender for shutdown signal
}

impl TranslationService {
    pub fn new(
        id: TranslationServiceId,
        events: &HMJEventBus,
        translation_lang_code: String,
        enabled: bool,
    ) -> Self {
        // Set up eventbus send
        let event_tx = events.translation.clone();

        // Subscribe to other events
        let history_rx = events.convo.subscribe();

        // Set up async channel
        let (translation_tx, translation_rx) =
            mpsc::channel::<(usize, Option<String>, TranslationServiceId)>(16);

        // Set up tokio translation runtime
        let translation_runtime =
            tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for translation");

        // Set up shutdown listener
        let (shutdown_tx, _) = broadcast::channel(1);

        // Set up translation language
        let translation_language = TranslationLanguage::from(translation_lang_code);

        println!(
            "Created TranslationService for {:?}, with enabled = {}",
            translation_language, enabled
        );

        Self {
            id,
            translate: Arc::new(Translate::default()),
            translation_language,
            enabled,
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

    // Translation services listens for a new ConvoItem and then attempts to translate the message.
    fn process_events(&mut self) {
        // Skip processing if translation is disabled
        if !self.enabled {
            return;
        }

        while let Ok(event) = self.history_rx.try_recv() {
            if let ConvoEvent::ItemAdded(key, item) = event {
                if item.translation.is_none() && !item.message.trim().is_empty() {
                    self.request_translation(
                        key,
                        item.message.clone(),
                        self.translate.clone(),
                        self.translation_language.clone(),
                    );
                }
            }
        }
    }

    // Spawns a new async task to translate the message.
    fn request_translation(
        &mut self,
        key: usize,
        msg: String,
        translate: Arc<Translate>,
        translation_type: TranslationLanguage,
    ) {
        if let Some(runtime) = &self.runtime {
            let tx = self.translation_tx.clone();
            let mut shutdown_rx = self.shutdown_tx.subscribe();
            let source_id = self.id;
            runtime.spawn(async move {
                // Spawn shutdown future
                let shutdown = async {
                    let _ = shutdown_rx.recv().await;
                };

                let task = async {
                    let (translation, source) = translate
                        .get_translation(&msg, "auto", translation_type.code(), source_id)
                        .await;
                    let _ = tx.send((key, translation, source)).await;
                };

                tokio::select! {
                    _ = shutdown => {
                        println!(".....Translation task received shutdown signal");
                    }
                    _ = task => {
                        println!("Translation task completed normally")
                    }
                }
            });
        }
    }

    // Receive completed translations, update convo by emitting a TranslationEvent::ItemTranslated event.
    fn receive_translations(&mut self) {
        while let Ok((key, translation, source)) = self.translation_rx.try_recv() {
            // Filter out translations initiated by other translation sources
            if source != self.id {
                return;
            }

            let _ = self.event_tx.send(TranslationEvent::ItemTranslated {
                key,
                translation,
                is_2nd_translation: matches!(source, TranslationServiceId::Second),
            });
        }
    }

    // Set translation types
    pub fn set_to_english(&mut self) {
        self.translation_language = TranslationLanguage::English;
    }
    pub fn set_to_french(&mut self) {
        self.translation_language = TranslationLanguage::French;
    }
    pub fn set_to_korean(&mut self) {
        self.translation_language = TranslationLanguage::Korean;
    }
    pub fn set_to_spanish(&mut self) {
        self.translation_language = TranslationLanguage::Spanish;
    }

    // Toggle translation on/off
    pub fn toggle_enabled(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
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

// The destination language for translation.
#[derive(Debug, Clone, PartialEq)]
pub enum TranslationLanguage {
    English,
    French,
    Korean,
    Spanish,
}

impl TranslationLanguage {
    pub fn code(&self) -> &'static str {
        match self {
            TranslationLanguage::English => "en",
            TranslationLanguage::French => "fr",
            TranslationLanguage::Korean => "ko",
            TranslationLanguage::Spanish => "es",
        }
    }
}

impl<T> From<T> for TranslationLanguage
where
    T: AsRef<str>,
{
    fn from(s: T) -> Self {
        let s = s.as_ref().trim();

        if s.eq_ignore_ascii_case("en") {
            TranslationLanguage::English
        } else if s.eq_ignore_ascii_case("fr") {
            TranslationLanguage::French
        } else if s.eq_ignore_ascii_case("ko") {
            TranslationLanguage::Korean
        } else if s.eq_ignore_ascii_case("es") {
            TranslationLanguage::Spanish
        } else {
            TranslationLanguage::English
        }
    }
}

// The DeepLX translation API entry point.
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
    // Get a translation from the DeepLX API.
    pub async fn get_translation(
        &self,
        input: &str,
        source_lang: &str,
        target_lang: &str,
        source_id: TranslationServiceId,
    ) -> (Option<String>, TranslationServiceId) {
        let result = self
            .translator
            .translate(source_lang, target_lang, input, None, None)
            .await;

        match result {
            Ok(res) => (Some(res.data.trim_end().to_owned()), source_id),
            Err(e) => {
                eprintln!("Error in Translate: {}", e);
                (None, source_id)
            }
        }
    }
}
