//! OpenAI API module
//!
//! Implements AIProvider for the OpenAI Responses API.

pub mod types;

use crate::openai::types::{
    request_helpers,
    response::{MessageContent, OutputItem, ResponseObject},
};
use crate::services::ai::{AIMessage, AIProvider};
use crate::settings::OpenAIProviderConfig;

use async_openai::{config::OpenAIConfig, types::responses, Client};
use std::error::Error;
use tokio::sync::{broadcast, mpsc};

pub struct OpenAIProvider {
    model: String,
    schema_desc: Option<String>,
    strict: bool,
    client: Client<OpenAIConfig>,
}

impl OpenAIProvider {
    pub fn new(config: &OpenAIProviderConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone().unwrap_or_default())
            .with_api_base(config.url.clone());

        Self {
            model: config.model.clone(),
            schema_desc: config.schema_description.clone(),
            strict: config.strict_request_object_adherence,
            client: Client::with_config(openai_config),
        }
    }
}

impl AIProvider for OpenAIProvider {
    fn spawn_request(
        &self,
        messages: Vec<AIMessage>,
        tx: mpsc::Sender<Option<String>>,
        rthandle: &tokio::runtime::Handle,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let model = self.model.clone();

        // instructions = messages[0] (system prompt + personality)
        let instructions = messages.first().map(|m| m.content.clone());

        // input = messages[1..] joined by newlines (already formatted as "Author: text")
        let input = messages
            .get(1..)
            .unwrap_or(&[])
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let schema_desc = self.schema_desc.clone();
        let strict = self.strict;
        let client = self.client.clone();

        rthandle.spawn(async move {
            println!("OpenAIProvider: Send task created");

            let shutdown = async {
                let _ = shutdown_rx.recv().await;
            };

            let task = async {
                let request =
                    generate_request(input, instructions, schema_desc, model, strict, false);

                let request = match request {
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

                        let text = response.output.iter().find_map(|item| {
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

                        let message = text.map(|raw| {
                            // Strip any control-token prefix (e.g. "<|channel|>final <|constrain|>JSON<|message|>")
                            // that OpenAI's Responses API leaks before the JSON payload.
                            let json_str = raw.find('{').map(|i| &raw[i..]).unwrap_or(&raw);
                            serde_json::from_str::<crate::openai::types::ConvoResponse>(json_str)
                                .map(|r| r.message)
                                .unwrap_or_else(|_| {
                                    eprintln!("OpenAIProvider: failed to parse ConvoResponse JSON, using raw text");
                                    raw
                                })
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
#[allow(clippy::too_many_arguments)]
fn generate_request(
    content: String,
    prompt: Option<String>,
    schema_description: Option<String>,
    model: String,
    strict: bool,
    streaming: bool,
) -> Result<responses::CreateResponse, Box<dyn Error + Send + Sync>> {
    let text_config = request_helpers::response_text_params_for_convo_schema(schema_description);
    let reasoning_config = request_helpers::reasoning_config();

    let mut request = if !strict {
        // Workaround: append instructions to input because the "instructions"
        // field isn't supported in LMStudio.
        let text = serde_json::to_string(&text_config).unwrap_or_default();
        let content = if let Some(prompt) = prompt {
            format!(
                "input : {}\ninstructions : {}\n text: {}",
                content, prompt, text
            )
        } else {
            format!("input : {}\n text: {}", content, text)
        };

        responses::CreateResponseArgs::default()
            .model(model)
            .input(content)
            .reasoning(reasoning_config)
            .build()?
    } else {
        responses::CreateResponseArgs::default()
            .model(model)
            .input(content)
            .instructions(prompt.unwrap_or_default())
            .reasoning(reasoning_config)
            .text(text_config)
            .build()?
    };

    if streaming {
        request.stream = Some(true);
    }

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
