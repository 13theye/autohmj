// src/events/mod.rs
//
// Event types

use crate::models::HistoryItem;

pub enum HistoryEvent {
    ItemAdded(usize, HistoryItem), // key, HistoryItem
    ItemTranslated(usize),         // key
    ItemAnimationCompleted(usize), // key
}
