// lib.rs
//
// data structures shared between Auto-HMJ client and server

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type History = BTreeMap<usize, HistoryItem>;

#[derive(Deserialize, Serialize)]
pub struct HistoryWrapper(pub History);

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
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}
impl HistoryItem {
    pub fn new(author: &str, message: &str) -> Self {
        Self {
            author: author.to_owned(),
            message: message.to_owned(),
            translation: None,
        }
    }
}
