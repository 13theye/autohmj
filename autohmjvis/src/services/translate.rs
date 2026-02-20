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
pub enum TranslationLanguageSlot {
    First,
    Second,
}

// Controller module for translations
pub struct TranslationService {
    translate: Arc<Translate>, // Translation API entry point

    // First language slot
    pub first_language: TranslationLanguage,
    pub first_enabled: bool,

    // Second language slot
    pub second_language: TranslationLanguage,
    pub second_enabled: bool,

    // Single shared async channel — TranslationServiceId in payload routes results
    pub runtime: Option<tokio::runtime::Runtime>,
    translation_tx: mpsc::Sender<(usize, Option<String>, TranslationLanguageSlot)>,
    translation_rx: mpsc::Receiver<(usize, Option<String>, TranslationLanguageSlot)>,

    // Events channel
    event_tx: broadcast::Sender<TranslationEvent>,
    history_rx: broadcast::Receiver<ConvoEvent>,

    // Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl TranslationService {
    pub fn new(
        events: &HMJEventBus,
        first_lang_code: String,
        first_enabled: bool,
        second_lang_code: String,
        second_enabled: bool,
    ) -> Self {
        let event_tx = events.translation.clone();
        let history_rx = events.convo.subscribe();

        let (translation_tx, translation_rx) =
            mpsc::channel::<(usize, Option<String>, TranslationLanguageSlot)>(16);

        let translation_runtime =
            tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for translation");

        let (shutdown_tx, _) = broadcast::channel(1);

        let first_language = TranslationLanguage::from(first_lang_code);
        let second_language = TranslationLanguage::from(second_lang_code);

        println!(
            "Created TranslationService: first={:?} (enabled={}), second={:?} (enabled={})",
            first_language, first_enabled, second_language, second_enabled
        );

        Self {
            translate: Arc::new(Translate::default()),
            first_language,
            first_enabled,
            second_language,
            second_enabled,
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

    // Listen for new ConvoItems and spawn a translation task for each enabled language slot.
    fn process_events(&mut self) {
        while let Ok(event) = self.history_rx.try_recv() {
            if let ConvoEvent::ItemAdded(key, item) = event {
                if item.message.trim().is_empty() {
                    continue;
                }
                if self.first_enabled && item.translation.is_none() {
                    self.request_translation(
                        key,
                        item.message.clone(),
                        self.translate.clone(),
                        self.first_language.clone(),
                        TranslationLanguageSlot::First,
                    );
                }
                if self.second_enabled && item.translation2.is_none() {
                    self.request_translation(
                        key,
                        item.message.clone(),
                        self.translate.clone(),
                        self.second_language.clone(),
                        TranslationLanguageSlot::Second,
                    );
                }
            }
        }
    }

    // Spawns a new async task to translate the message for the given language slot.
    fn request_translation(
        &mut self,
        key: usize,
        msg: String,
        translate: Arc<Translate>,
        translation_type: TranslationLanguage,
        slot: TranslationLanguageSlot,
    ) {
        if let Some(runtime) = &self.runtime {
            let tx = self.translation_tx.clone();
            let mut shutdown_rx = self.shutdown_tx.subscribe();
            runtime.spawn(async move {
                let shutdown = async {
                    let _ = shutdown_rx.recv().await;
                };

                let task = async {
                    let (translation, source) = translate
                        .get_translation(&msg, "auto", translation_type.code(), slot)
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

    // Receive completed translations and emit TranslationEvent::ItemTranslated.
    // The TranslationServiceId in the payload determines which slot (first or second) was translated.
    fn receive_translations(&mut self) {
        while let Ok((key, translation, source)) = self.translation_rx.try_recv() {
            let _ = self.event_tx.send(TranslationEvent::ItemTranslated {
                key,
                translation,
                is_2nd_translation: matches!(source, TranslationLanguageSlot::Second),
            });
        }
    }

    // Toggle/set the first language slot enabled state
    pub fn toggle_first_enabled(&mut self) {
        self.first_enabled = !self.first_enabled;
        println!("First language slot enabled: {}", self.first_enabled);
    }

    pub fn toggle_second_enabled(&mut self) {
        self.second_enabled = !self.second_enabled;
        println!("Second language slot enabled: {}", self.second_enabled);
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
        source_id: TranslationLanguageSlot,
    ) -> (Option<String>, TranslationLanguageSlot) {
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
