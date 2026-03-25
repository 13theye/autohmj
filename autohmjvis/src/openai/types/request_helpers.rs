// Helpers for generating OpenAI Response types described in the crate async-openai

use super::ConvoResponse;

use async_openai::types::responses::{
    Reasoning, ReasoningEffort, ResponseFormatJsonSchema, ResponseTextParam,
    TextResponseFormatConfiguration,
};
use schemars::schema_for;

/// Helper function to allow default generation of ResponseTextParam for this app.
pub fn response_text_params_for_convo_schema(
    schema_description: Option<String>,
) -> ResponseTextParam {
    ResponseTextParam {
        format: TextResponseFormatConfiguration::JsonSchema(convo_response_json_schema(
            schema_description,
        )),
        verbosity: None,
    }
}

/// Helper function for generating a ResponseFormatJsonSchema for this app.
fn convo_response_json_schema(description: Option<String>) -> ResponseFormatJsonSchema {
    let schema = Some(schema_for!(ConvoResponse).into());

    ResponseFormatJsonSchema {
        description,
        name: "convo_response_object_schema".to_owned(),
        schema,
        strict: Some(true),
    }
}

/// Helper function to allow default generation of ReasoningConfig for this app.
pub fn reasoning_config() -> Reasoning {
    Reasoning {
        effort: Some(ReasoningEffort::Medium),
        summary: None,
    }
}
