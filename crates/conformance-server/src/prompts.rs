use rust_mcp_sdk::schema::{
    ContentBlock, GetPromptResult, Prompt, PromptArgument, PromptMessage, Role,
};

const IMAGE_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

fn user_message(content: ContentBlock) -> PromptMessage {
    PromptMessage { role: Role::User, content }
}

// ---------------
// 1. test_simple_prompt
// ---------------
pub struct TestSimplePrompt;

impl TestSimplePrompt {
    pub fn prompt() -> Prompt {
        Prompt {
            name: "test_simple_prompt".into(),
            description: Some("A simple prompt for conformance testing.".into()),
            arguments: vec![],
            icons: vec![],
            meta: None,
            title: None,
        }
    }

    pub fn get_prompt() -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        Ok(GetPromptResult {
            messages: vec![user_message(ContentBlock::text_content("This is a simple prompt for testing.".to_string()))],
            meta: None,
            description: Some("This is a simple prompt for testing.".into()),
        })
    }
}

// ---------------
// 2. test_prompt_with_arguments
// ---------------
pub struct TestPromptWithArguments;

impl TestPromptWithArguments {
    pub fn prompt() -> Prompt {
        Prompt {
            name: "test_prompt_with_arguments".into(),
            description: Some("A parameterized prompt for conformance testing.".into()),
            arguments: vec![
                PromptArgument {
                    name: "arg1".into(),
                    description: Some("First test argument".into()),
                    required: Some(true),
                    title: None,
                },
                PromptArgument {
                    name: "arg2".into(),
                    description: Some("Second test argument".into()),
                    required: Some(true),
                    title: None,
                },
            ],
            icons: vec![],
            meta: None,
            title: None,
        }
    }

    pub fn get_prompt(
        arg1: &str,
        arg2: &str,
    ) -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        Ok(GetPromptResult {
            messages: vec![user_message(ContentBlock::text_content(format!(
                "Prompt with arguments: arg1='{arg1}', arg2='{arg2}'"
            )))],
            meta: None,
            description: Some(format!(
                "Prompt with arguments: arg1='{arg1}', arg2='{arg2}'"
            )),
        })
    }
}

// ---------------
// 3. test_prompt_with_embedded_resource
// ---------------
pub struct TestPromptWithEmbeddedResource;

impl TestPromptWithEmbeddedResource {
    pub fn prompt() -> Prompt {
        Prompt {
            name: "test_prompt_with_embedded_resource".into(),
            description: Some("A prompt with embedded resource for conformance testing.".into()),
            arguments: vec![PromptArgument {
                name: "resourceUri".into(),
                description: Some("URI of the resource to embed".into()),
                required: Some(true),
                title: None,
            }],
            icons: vec![],
            meta: None,
            title: None,
        }
    }

    pub fn get_prompt(
        resource_uri: &str,
    ) -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        Ok(GetPromptResult {
            messages: vec![
                user_message(ContentBlock::embedded_text_resource(
                    resource_uri,
                    "text/plain",
                    "Embedded resource content for testing.",
                )),
                user_message(ContentBlock::text_content("Please process the embedded resource above.".to_string())),
            ],
            meta: None,
            description: Some("A prompt with embedded resource for conformance testing.".into()),
        })
    }
}

// ---------------
// 4. test_prompt_with_image
// ---------------
pub struct TestPromptWithImage;

impl TestPromptWithImage {
    pub fn prompt() -> Prompt {
        Prompt {
            name: "test_prompt_with_image".into(),
            description: Some("A prompt with image content for conformance testing.".into()),
            arguments: vec![],
            icons: vec![],
            meta: None,
            title: None,
        }
    }

    pub fn get_prompt() -> Result<GetPromptResult, rust_mcp_sdk::schema::RpcError> {
        Ok(GetPromptResult {
            messages: vec![
                user_message(ContentBlock::image_content(IMAGE_BASE64.to_string(), "image/png".to_string())),
                user_message(ContentBlock::text_content("Please analyze the image above.".to_string())),
            ],
            meta: None,
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
