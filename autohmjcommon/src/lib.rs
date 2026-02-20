// lib.rs
//
// data structures shared between Auto-HMJ client and server

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Conversation = BTreeMap<usize, ConvoItem>; // <key, ConvoItem>

// A simple wrapper type for the full conversation history to aid serialization
#[derive(Deserialize, Serialize)]
pub struct ConvoWrapper(pub Conversation);

// Wrapper for HMJ Client/Server message: (client_id, message_text, Option<cursor_position>)
// Human input in the client is turned into this struct before being sent to the server.
#[derive(Deserialize, Serialize)]
pub struct HMJMessageWrapper {
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_position: Option<usize>,
}

impl HMJMessageWrapper {
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

// When the server receives a HMJMessage, it is turned into a ConvoItem for storage in the
// conversation history.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ConvoItem {
    pub author: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    pub translation2: Option<String>,
}
impl ConvoItem {
    pub fn new(author: &str, message: &str) -> Self {
        let message = if message.ends_with('\n') {
            message.trim_end_matches('\n')
        } else {
            message
        };
        Self {
            author: author.to_owned(),
            message: message.to_owned(),
            translation: None,
            translation2: None,
        }
    }
}

// UI Commands for client/vis settings change communication
#[derive(Debug, Deserialize, Serialize)]
pub enum CommandMessage {
    AISend(String),
    OscLeftSetting(bool),
    OscRightSetting(bool),
    OscHumanSetting(bool),
    ClearGrid(String),
    ResetConversation,
}
