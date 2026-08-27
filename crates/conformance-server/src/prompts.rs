//! Conformance prompt fixtures.
//!
//! Every prompt is declared with the SDK's `mcp_prompt!` macro, which
//! generates the `Prompt` value (name, description, arguments) and the
//! `from_arguments` parser. Template prompts use the `messages` attribute and
//! `render()`; prompts that produce non-template content (embedded resources,
//! images) keep a hand-written `get_prompt` builder for the response.

use rust_mcp_sdk::macros::mcp_prompt;
use rust_mcp_sdk::schema::{
    ContentBlock, EmbeddedResourceResource, GetPromptResult, Prompt, PromptMessage, Role,
    TextResourceContents,
};

const IMAGE_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

fn user_message(content: ContentBlock) -> PromptMessage {
    PromptMessage {
        role: Role::User,
        content,
    }
}

// ---------------
// 1. test_simple_prompt
// ---------------
#[mcp_prompt(
    name = "test_simple_prompt",
    description = "A simple prompt for conformance testing.",
    messages = [(role = "user", content = "This is a simple prompt for testing.")]
)]
pub struct TestSimplePrompt {}

impl TestSimplePrompt {
    pub fn get_prompt() -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        Ok(Self::from_arguments(None)?.render())
    }
}

// ---------------
// 2. test_prompt_with_arguments
// ---------------
#[mcp_prompt(
    name = "test_prompt_with_arguments",
    description = "A parameterized prompt for conformance testing.",
    messages = [
        (role = "user", content = "Prompt with arguments: arg1='{arg1}', arg2='{arg2}'"),
    ]
)]
pub struct TestPromptWithArguments {
    #[prompt_argument(description = "First test argument")]
    pub arg1: String,
    #[prompt_argument(description = "Second test argument")]
    pub arg2: String,
}

impl TestPromptWithArguments {
    pub fn get_prompt(
        arg1: &str,
        arg2: &str,
    ) -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        let prompt = Self::from_arguments(Some(&std::collections::BTreeMap::from([
            ("arg1".to_string(), arg1.to_string()),
            ("arg2".to_string(), arg2.to_string()),
        ])))?;
        Ok(prompt.render())
    }
}

// ---------------
// 3. test_prompt_with_embedded_resource
// ---------------
#[mcp_prompt(
    name = "test_prompt_with_embedded_resource",
    description = "A prompt with embedded resource for conformance testing."
)]
#[derive(::serde::Serialize, ::serde::Deserialize)]
pub struct TestPromptWithEmbeddedResource {
    #[prompt_argument(description = "URI of the resource to embed")]
    #[serde(rename = "resourceUri")]
    pub resource_uri: String,
}

impl TestPromptWithEmbeddedResource {
    pub fn get_prompt(
        resource_uri: &str,
    ) -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        Ok(GetPromptResult {
            messages: vec![
                user_message(ContentBlock::embedded_resource(
                    EmbeddedResourceResource::TextResourceContents(
                        TextResourceContents::new(
                            "Embedded resource content for testing.",
                            resource_uri,
                        )
                        .with_mime_type("text/plain".to_string()),
                    ),
                )),
                user_message(ContentBlock::text_content(
                    "Please process the embedded resource above.".to_string(),
                )),
            ],
            meta: None,
            result_type: "complete".to_string(),
            description: Some("A prompt with embedded resource for conformance testing.".into()),
        })
    }
}

// ---------------
// 4. test_prompt_with_image
// ---------------
#[mcp_prompt(
    name = "test_prompt_with_image",
    description = "A prompt with image content for conformance testing."
)]
pub struct TestPromptWithImage {}

impl TestPromptWithImage {
    pub fn get_prompt() -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        Ok(GetPromptResult {
            messages: vec![
                user_message(ContentBlock::image_content(
                    IMAGE_BASE64.to_string(),
                    "image/png".to_string(),
                )),
                user_message(ContentBlock::text_content(
                    "Please analyze the image above.".to_string(),
                )),
            ],
            meta: None,
            result_type: "complete".to_string(),
            description: Some("A prompt with image content for conformance testing.".into()),
        })
    }
}

// ---------------
// Prompt list
// ---------------
pub fn all_prompts() -> Vec<Prompt> {
    vec![
        TestSimplePrompt::prompt(),
        TestPromptWithArguments::prompt(),
        TestPromptWithEmbeddedResource::prompt(),
        TestPromptWithImage::prompt(),
    ]
}
