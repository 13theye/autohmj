// src/model/structs.rs
//
// data structures for Auto-HMJ

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize)]
pub struct History(pub HashMap<usize, HistoryItem>);

#[derive(Clone, Serialize)]
pub struct HistoryItem {
    pub author: String,
    pub msg: String,
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

fn translate(msg: &str) -> String {
    // sends message to web translation API, returns the translated String
    todo!();
}
