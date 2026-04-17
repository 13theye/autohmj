//! src/content/content_event.rs
//!
//! Events emitted by the ContentManager

use crate::content::content_manager::KeyedConvoItem;

#[derive(Clone, Debug)]
pub enum ContentEvent {
    UpdatedLatest {
        source_id: String,
        convo_item: Option<KeyedConvoItem>,
    },
    UpdatedTranslation {
        source_id: String,
        convo_item: Option<KeyedConvoItem>,
    },
    UpdatedLiveInput {
        source_id: String,
        content: String,
    },
    AIRequested(String),
    AIFailed(String),
}
