//! OpenAI API module
//!
//! This module provides a fire-and-poll synchronous facade backed by
//! Tolio runtime and reqwest.

pub mod types;

use crate::openai::types::{request_helpers, response::ResponseObject};

use async_openai::{config::OpenAIConfig, types::responses, Client};
use std::error::Error;
use tokio::sync::{broadcast, mpsc};

pub struct OpenAIService {
    // LLM system prompt
    pub prompt: String,
    // Response schema description:
    pub schema_desc: Option<String>,

    //Model name
    pub model: String,

    // Strict adherence to OpenAI API request schema
    // Set to false when using LMStudio
    pub strict_request_object_adherence: bool,

    // Tokio runtime handle
    rthandle: tokio::runtime::Handle,

    // Response task channel
    response_tx: mpsc::Sender<ResponseObject>,
    response_rx: mpsc::Receiver<ResponseObject>,

    // Async_openai client
    // OpenAIConfig contains api key & base URL
    client: Client<OpenAIConfig>,
}
