// lib.rs
//
// data structures for Auto-HMJ

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize)]
pub struct History(pub BTreeMap<usize, HistoryItem>);

// Wrapper for HMJ Client/Server message: (client_id, message_text)
#[derive(Deserialize, Serialize)]
pub struct HMJMessage(pub String, pub String);

impl HMJMessage {
    pub fn serialize(&self) -> String {
        let result = serde_json::to_string(&self);
        match result {
            Ok(string) => string,
            Err(e) => {
                eprintln!("Error serializing HMJMessage: {}", e);
                String::new()
            }
        }
    }
}

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
