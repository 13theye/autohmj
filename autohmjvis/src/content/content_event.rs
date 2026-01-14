//! src/content/content_event.rs
//!
//! Events emitted by the ContentManager

use crate::content::content_manager::KeyedConvoItem;

#[derive(Clone, Debug)]
pub enum ContentEvent {
    UpdatedLatest(Option<KeyedConvoItem>),
    UpdatedLiveInput(String, String),
}
