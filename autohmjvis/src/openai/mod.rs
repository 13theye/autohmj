//! OpenAI API module
//!
//! Implements AIProvider for the OpenAI Responses API.

pub mod types;

use crate::openai::types::{
    request_helpers,
    response::{MessageContent, OutputItem, ResponseObject},
};
use crate::services::ai::{AIContext, AIProvider};
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

        println!("Starting OpenAIProvider...");

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

                        println!("Response body: {:#?}", response);

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

        // Enable this if using structured responses
        let _text = serde_json::to_string(&text_config).unwrap_or_default();

        let content = if let Some(prompt) = prompt {
            // Uncomment the following if using structures responses
            /*format!(
                "input : {}\ninstructions : {}\n text: {}",
                content, prompt, text
            )*/

            // Uncomment the following if not using structured responses
            format!("input : {}\ninstructions : {}", content, prompt)
        } else {
            // Uncomment the following if using structures responses
            //format!("input : {}\n text: {}", content, text)

            // Uncomment the following if not using structured responses
            format!("input : {}", content)
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
