//! OpenAI API module
//!
//! Implements AIProvider for the OpenAI Responses API.

pub mod types;

use crate::openai::types::response::{MessageContent, OutputItem, ResponseObject};
use crate::services::ai::{AIContext, AIProvider, ProviderOutput};
use crate::settings::OpenAIProviderConfig;

use async_openai::{
    config::OpenAIConfig,
    types::responses::{self, Reasoning, ReasoningEffort},
    Client,
};
use std::error::Error;
use tokio::sync::{broadcast, mpsc};

pub struct OpenAIProvider {
    model: String,
    client: Client<OpenAIConfig>,
}

impl OpenAIProvider {
    pub fn new(config: &OpenAIProviderConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone().unwrap_or_default())
            .with_api_base(config.url.clone());

        println!("Starting OpenAIProvider...");

        Self {
            model: config.model.clone(),
            client: Client::with_config(openai_config),
        }
    }
}

impl AIProvider for OpenAIProvider {
    fn spawn_request(
        &self,
        context: AIContext,
        tx: mpsc::Sender<Option<ProviderOutput>>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let model = self.model.clone();

        let (instructions, history_str, new_item_str, previous_response_id) = match context {
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
                (ctx.instructions, history_str, new_item_str, ctx.previous_response_id)
            }
            AIContext::Moderator(ctx) => {
                let history_str = ctx
                    .history
                    .iter()
                    .map(|msg| format!("{}: {}", msg.author, msg.content))
                    .collect::<Vec<_>>()
                    .join("\n");
                (ctx.instructions, history_str, None, ctx.previous_response_id)
            }
        };

        let client = self.client.clone();

        rthandle.spawn(async move {
            println!("OpenAIProvider: Send task created");

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
                    ),
                    None => generate_stateless_request(
                        history_str,
                        new_item_str,
                        instructions,
                        model,
                    ),
                };

                let request = match request_result {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("OpenAIProvider: Failed to generate request: {}", e);
                        let _ = tx.send(None).await;
                        return;
                    }
                };

                match send_response_request(request, &client).await {
                    Ok(response) => {
                        println!(
                            "OpenAIProvider: Received successful response with id: {}",
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
                                let _ = tx.send(Some(ProviderOutput {
                                    message,
                                    response_id: Some(response.id.clone()),
                                })).await;
                            }
                            None => {
                                let _ = tx.send(None).await;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("OpenAIProvider: API error: {}", e);
                        let _ = tx.send(None).await;
                    }
                }
            };

            tokio::select! {
                _ = shutdown => {
                    println!("...OpenAIProvider received shutdown signal");
                }
                _ = task => {
                    println!("...OpenAIProvider task completed normally");
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

/// Shared low-level builder. Sets reasoning config, disables streaming,
/// and optionally links to a previous response for stateful conversations.
fn build_request(
    content: String,
    model: String,
    previous_response_id: Option<String>,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let reasoning = Reasoning {
        effort: Some(ReasoningEffort::Medium),
        summary: None,
    };

    let mut request = responses::CreateResponseArgs::default()
        .model(model)
        .input(content)
        .reasoning(reasoning)
        .build()?;

    request.previous_response_id = previous_response_id;
    // Streaming is not currently used but kept here for future use.
    request.stream = Some(false);

    println!("OpenAIProvider: Request object:\n{:#?}", request);

    Ok(request)
}

/// First turn or post-reset: sends the full conversation history.
/// No `previous_response_id` — the server has no prior context.
fn generate_stateless_request(
    history: String,
    new_item: Option<String>,
    instructions: String,
    model: String,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let content = format_input(history, new_item, &instructions);
    build_request(content, model, None)
}

/// Subsequent turns: sends only messages added since the last response.
/// Includes `previous_response_id` so the server can link to its prior context.
fn generate_stateful_request(
    incremental: String,
    new_item: Option<String>,
    instructions: String,
    model: String,
    previous_response_id: String,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let content = format_input(incremental, new_item, &instructions);
    build_request(content, model, Some(previous_response_id))
}

async fn send_response_request(
    request: responses::CreateResponse,
    client: &Client<OpenAIConfig>,
) -> Result<ResponseObject, Box<dyn Error + Send + Sync>> {
    let response: ResponseObject = client.responses().create_byot(request).await?;
    Ok(response)
}
