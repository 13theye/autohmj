// lib.rs
//
// data structures for Auto-HMJ

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize)]
pub struct History(pub BTreeMap<usize, HistoryItem>);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryItem {
    pub author: String,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}
impl HistoryItem {
    pub fn new(author: &str, msg: &str) -> Self {
        Self {
            author: author.to_owned(),
            msg: msg.to_owned(),
            translation: None,
        }
    }
}
