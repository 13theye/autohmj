// src/services/translate.rs
//
// A Translation service that uses the external DeepLX crate to access DeepLX translation
//
//
use deeplx::{Config, DeepLX};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::events::{ConvoEvent, HMJEventBus};

// ===== Provider trait =====

/// Interface between TranslationService and a specific translation backend.
/// Mirrors the AIProvider pattern — implementors spawn async tasks and send
/// results back through `tx`.
pub trait TranslationProvider: Send + Sync + 'static {
    /// Spawn async task(s) that produce one `(key, translation, slot)` result
    /// per entry in `slots`, sent through `tx`.
    ///
    /// `history` is the full conversation at the time of the request, including
    /// the message to translate as the last entry. Providers that use context
    /// (e.g. AI) can use it to produce more accurate translations.
    fn spawn_translation(
        &self,
        key: usize,
        msg: String,
        history: Vec<(String, String)>,
        slots: Vec<(TranslationLanguageSlot, TranslationLanguage)>,
        tx: mpsc::Sender<(usize, Option<String>, TranslationLanguageSlot)>,
        rthandle: &tokio::runtime::Handle,
        shutdown_rx: broadcast::Receiver<()>,
    );
}

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
    provider: Box<dyn TranslationProvider>, // Translation backend

    // Conversation history accumulated for context-aware providers
    conversation_history: Vec<(String, String)>, // (author, message)

    // First language slot
    pub first_language: TranslationLanguage,
    pub first_enabled: bool,

    // Second language slot
    pub second_language: TranslationLanguage,
    pub second_enabled: bool,

    // Tokio runtime handle
    rthandle: tokio::runtime::Handle,

    // Single shared async channel — TranslationServiceId in payload routes results
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
        provider: Box<dyn TranslationProvider>,
        rthandle: tokio::runtime::Handle,
    ) -> Self {
        let event_tx = events.translation.clone();
        let history_rx = events.convo.subscribe();

        let (translation_tx, translation_rx) =
            mpsc::channel::<(usize, Option<String>, TranslationLanguageSlot)>(16);

        let (shutdown_tx, _) = broadcast::channel(1);

        let first_language = TranslationLanguage::from(first_lang_code);
        let second_language = TranslationLanguage::from(second_lang_code);

        println!(
            "Created TranslationService: first={:?} (enabled={}), second={:?} (enabled={})",
            first_language, first_enabled, second_language, second_enabled
        );

        Self {
            provider,
            conversation_history: Vec::new(),
            first_language,
            first_enabled,
            second_language,
            second_enabled,
            rthandle,
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

    // Listen for new ConvoItems and collect enabled slots, then delegate to the provider.
    fn process_events(&mut self) {
        while let Ok(event) = self.history_rx.try_recv() {
            if let ConvoEvent::ItemAdded(key, item) = event {
                if item.message.trim().is_empty() {
                    continue;
                }
                // Append to history before spawning so providers receive full context
                // including the message to be translated as the last entry.
                self.conversation_history.push((item.author.clone(), item.message.clone()));
                let mut slots = Vec::new();
                if self.first_enabled && item.translation.is_none() {
                    slots.push((TranslationLanguageSlot::First, self.first_language.clone()));
                }
                if self.second_enabled && item.translation2.is_none() {
                    slots.push((TranslationLanguageSlot::Second, self.second_language.clone()));
                }
                if !slots.is_empty() {
                    self.provider.spawn_translation(
                        key,
                        item.message.clone(),
                        self.conversation_history.clone(),
                        slots,
                        self.translation_tx.clone(),
                        &self.rthandle,
                        self.shutdown_tx.subscribe(),
                    );
                }
            }
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

// ===== DeepLX provider =====

/// Translation backend that uses the DeepLX API.
/// Spawns one async task per language slot (preserving the original behavior).
pub struct DeepLXProvider {
    translate: Arc<Translate>,
}

impl Default for DeepLXProvider {
    fn default() -> Self {
        Self {
            translate: Arc::new(Translate::default()),
        }
    }
}

impl TranslationProvider for DeepLXProvider {
    fn spawn_translation(
        &self,
        key: usize,
        msg: String,
        _history: Vec<(String, String)>,
        slots: Vec<(TranslationLanguageSlot, TranslationLanguage)>,
        tx: mpsc::Sender<(usize, Option<String>, TranslationLanguageSlot)>,
        rthandle: &tokio::runtime::Handle,
        shutdown_rx: broadcast::Receiver<()>,
    ) {
        for (slot, language) in slots {
            let translate = self.translate.clone();
            let tx = tx.clone();
            let msg = msg.clone();
            let mut task_shutdown_rx = shutdown_rx.resubscribe();

            rthandle.spawn(async move {
                let shutdown = async {
                    let _ = task_shutdown_rx.recv().await;
                };
                let task = async {
                    let (translation, source) = translate
                        .get_translation(&msg, "auto", language.code(), slot)
                        .await;
                    let _ = tx.send((key, translation, source)).await;
                };
                tokio::select! {
                    _ = shutdown => {
                        println!(".....Translation task received shutdown signal");
                    }
                    _ = task => {
                        println!("Translation task completed normally");
                    }
                }
            });
        }
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
