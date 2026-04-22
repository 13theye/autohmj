//! OpenAI API module for Local AI
//!
//! Implements AIProvider for the OpenAI Responses API.

pub mod types;

use crate::local_ai::types::response::{MessageContent, OutputItem, ResponseObject};
use crate::services::ai::{AIContext, AIProvider, ProviderOutput};
use crate::settings::LocalAIProviderConfig;

use async_openai::{
    config::OpenAIConfig,
    types::responses::{self, Reasoning, ReasoningEffort},
    Client,
};
use std::error::Error;
use tokio::sync::{broadcast, mpsc};

pub struct LocalAIProvider {
    model: String,
    client: Client<OpenAIConfig>,
    request_timeout: std::time::Duration,
    thinking_model: bool,
}

impl LocalAIProvider {
    pub fn new(config: &LocalAIProviderConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone().unwrap_or_default())
            .with_api_base(config.url.clone());

        println!("Starting LocalAIProvider...");

        Self {
            model: config.model.clone(),
            client: Client::with_config(openai_config),
            request_timeout: std::time::Duration::from_secs(config.request_timeout_secs),
            thinking_model: config.thinking_model,
        }
    }
}

impl AIProvider for LocalAIProvider {
    fn spawn_request(
        &self,
        context: AIContext,
        tx: mpsc::Sender<Option<ProviderOutput>>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let model = self.model.clone();

        let (instructions, history_str, new_item_str, previous_response_id, is_moderator) =
            match context {
                AIContext::Persona(ctx) => {
                    let history_str = ctx
                        .history
                        .iter()
                        .map(|msg| format!("{}: {}", msg.author, msg.content))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let new_item_str = ctx
                        .new_item
                        .map(|msg| format!("{}: {}", msg.author, msg.content));
                    (
                        ctx.instructions,
                        history_str,
                        new_item_str,
                        ctx.previous_response_id,
                        false,
                    )
                }
                AIContext::Moderator(ctx) => {
                    let history_str = ctx
                        .history
                        .iter()
                        .map(|msg| format!("{}: {}", msg.author, msg.content))
                        .collect::<Vec<_>>()
                        .join("\n");
                    (
                        ctx.instructions,
                        history_str,
                        None,
                        ctx.previous_response_id,
                        true,
                    )
                }
            };

        let client = self.client.clone();
        let request_timeout = self.request_timeout;
        let thinking_model = self.thinking_model;

        rthandle.spawn(async move {
            println!("LocalAIProvider: Send task created");

            let shutdown = async {
                let _ = shutdown_rx.recv().await;
            };

            let task = async {
                let request_result = match previous_response_id {
                    Some(prev_id) => generate_stateful_request(
                        history_str,
                        new_item_str,
                        instructions,
                        model,
                        prev_id,
                        is_moderator,
                        thinking_model,
                    ),
                    None => generate_stateless_request(
                        history_str,
                        new_item_str,
                        instructions,
                        model,
                        is_moderator,
                        thinking_model,
                    ),
                };

                let request = match request_result {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("LocalAIProvider: Failed to generate request: {}", e);
                        let _ = tx.send(None).await;
                        return;
                    }
                };

                match tokio::time::timeout(request_timeout, send_response_request(request, &client))
                    .await
                {
                    Err(_) => {
                        eprintln!(
                            "LocalAIProvider: Request timed out after {:?}",
                            request_timeout
                        );
                        let _ = tx.send(None).await;
                    }
                    Ok(Ok(response)) => {
                        println!(
                            "LocalAIProvider: Received successful response with id: {}",
                            response.id
                        );

                        println!("Response body: {:#?}", response);

                        let message = response.output.iter().find_map(|item| {
                            if let OutputItem::Message(msg) = item {
                                msg.content.iter().find_map(|c| {
                                    if let MessageContent::OutputText { text } = c {
                                        Some(text.clone())
                                    } else {
                                        None
                                    }
                                })
                            } else {
                                None
                            }
                        });

                        match message {
                            Some(message) => {
                                let _ = tx
                                    .send(Some(ProviderOutput {
                                        message,
                                        response_id: Some(response.id.clone()),
                                    }))
                                    .await;
                            }
                            None => {
                                let _ = tx.send(None).await;
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        eprintln!("LocalAIProvider: API error: {}", e);
                        let _ = tx.send(None).await;
                    }
                }
            };

            tokio::select! {
                _ = shutdown => {
                    println!("...LocalAIProvider received shutdown signal");
                }
                _ = task => {
                    println!("...LocalAIProvider task completed normally");
                }
            }
        });
    }
}

/// Combines history and optional new item into a single input string,
/// appending the instructions for compatibility with local inference servers
/// that do not support the top-level `instructions` field.
fn format_input(history: String, new_item: Option<String>, instructions: &str) -> String {
    let mut input = history;
    if let Some(item) = new_item {
        if !input.is_empty() {
            input.push('\n');
        }
        input.push_str(&item);
    }
    format!("input : {}\ninstructions : {}", input, instructions)
}

/// Shared low-level builder. Optionally sets reasoning config, disables streaming,
/// and optionally links to a previous response for stateful conversations.
fn build_request(
    content: String,
    model: String,
    previous_response_id: Option<String>,
    is_moderator: bool,
    thinking_model: bool,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let mut args = responses::CreateResponseArgs::default();
    args.model(model).input(content);

    if thinking_model {
        let reasoning = Reasoning {
            effort: Some(if is_moderator {
                ReasoningEffort::Minimal
            } else {
                ReasoningEffort::Medium
            }),
            summary: None,
        };
        args.reasoning(reasoning);
    }

    let mut request = args.build()?;

    request.previous_response_id = previous_response_id;
    // Streaming is not currently used but kept here for future use.
    request.stream = Some(false);

    println!("LocalAIProvider: Request object:\n{:#?}", request);

    Ok(request)
}

/// First turn or post-reset: sends the full conversation history.
/// No `previous_response_id` — the server has no prior context.
fn generate_stateless_request(
    history: String,
    new_item: Option<String>,
    instructions: String,
    model: String,
    is_moderator: bool,
    thinking_model: bool,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let content = format_input(history, new_item, &instructions);
    build_request(content, model, None, is_moderator, thinking_model)
}

/// Subsequent turns: sends only messages added since the last response.
/// Includes `previous_response_id` so the server can link to its prior context.
fn generate_stateful_request(
    incremental: String,
    new_item: Option<String>,
    instructions: String,
    model: String,
    previous_response_id: String,
    is_moderator: bool,
    thinking_model: bool,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let content = format_input(incremental, new_item, &instructions);
    build_request(content, model, Some(previous_response_id), is_moderator, thinking_model)
}

async fn send_response_request(
    request: responses::CreateResponse,
    client: &Client<OpenAIConfig>,
) -> Result<ResponseObject, Box<dyn Error + Send + Sync>> {
    let response: ResponseObject = client.responses().create_byot(request).await?;
    Ok(response)
}
