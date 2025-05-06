// src/events/types.rs
//
// Event types

use crate::models::HistoryItem;

#[derive(Clone)]
pub enum HistoryEvent {
    ItemAdded(usize, HistoryItem),         // key, HistoryItem
    ItemTranslated(usize, Option<String>), // key, translation
    ItemAnimationCompleted(usize),         // key
}
