//! src/content/content_event.rs
//!
//! Events emitted by the ContentManager

use crate::content::content_manager::KeyedConvoItem;

#[derive(Clone, Debug)]
pub enum ContentEvent {
    UpdatedLatest(String, Option<KeyedConvoItem>),
    UpdatedTranslation(String, Option<KeyedConvoItem>),
    UpdatedLiveInput(String, String),
    GemmaRequested(String),
}
