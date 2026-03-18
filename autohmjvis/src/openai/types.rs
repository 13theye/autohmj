pub mod request_helpers;
pub mod response;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An entry to be added to the end of the conversation history.
/// Author: the name of the speaker
/// Message: the text of the message
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConvoResponse {
    pub author: String,
    pub message: String,
}
