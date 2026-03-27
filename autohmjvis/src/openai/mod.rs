//! OpenAI API module
//!
//! Implements AIProvider for the OpenAI Responses API.

pub mod types;

use crate::openai::types::response::{MessageContent, OutputItem, ResponseObject};
use crate::services::ai::{AIContext, AIProvider};
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
        tx: mpsc::Sender<Option<String>>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let model = self.model.clone();

        let (instructions, input) = match context {
            AIContext::Persona(ctx) => {
                let mut lines: Vec<String> = ctx
                    .history
                    .iter()
                    .map(|msg| format!("{}: {}", msg.author, msg.content))
                    .collect();
                if let Some(new_msg) = ctx.new_item {
                    lines.push(format!("{}: {}", new_msg.author, new_msg.content));
                }
                (Some(ctx.instructions), lines.join("\n"))
            }
            AIContext::Moderator(ctx) => {
                let lines: Vec<String> = ctx
                    .history
                    .iter()
                    .map(|msg| format!("{}: {}", msg.author, msg.content))
                    .collect();
                (Some(ctx.instructions), lines.join("\n"))
            }
        };

        let client = self.client.clone();

        rthandle.spawn(async move {
            println!("OpenAIProvider: Send task created");

            let shutdown = async {
                let _ = shutdown_rx.recv().await;
            };

            let task = async {
                let request = match generate_request(input, instructions, model) {
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

                        let _ = tx.send(message).await;
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

/// Builds a Responses API request object.
///
/// Instructions are appended to the input field for compatibility with
/// local inference servers (e.g. LMStudio) that do not support the
/// top-level `instructions` field.
fn generate_request(
    content: String,
    prompt: Option<String>,
    model: String,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let reasoning = Reasoning {
        effort: Some(ReasoningEffort::Medium),
        summary: None,
    };

    let content = if let Some(prompt) = prompt {
        format!("input : {}\ninstructions : {}", content, prompt)
    } else {
        format!("input : {}", content)
    };

    let mut request = responses::CreateResponseArgs::default()
        .model(model)
        .input(content)
        .reasoning(reasoning)
        .build()?;

    // Streaming is not currently used but kept here for future use.
    request.stream = Some(false);

    println!("OpenAIProvider: Request object:\n{:#?}", request);

    Ok(request)
}

async fn send_response_request(
    request: responses::CreateResponse,
    client: &Client<OpenAIConfig>,
) -> Result<ResponseObject, Box<dyn Error + Send + Sync>> {
    let response: ResponseObject = client.responses().create_byot(request).await?;
    Ok(response)
}
