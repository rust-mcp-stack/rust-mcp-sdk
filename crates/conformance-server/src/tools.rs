use rust_mcp_macros::JsonSchema;
use rust_mcp_sdk::{
    macros::mcp_tool,
    schema::{
        schema_utils::CallToolError, AudioContent, CallToolResult, ContentBlock, EmbeddedResource,
        EmbeddedResourceResource, ImageContent, TextContent, TextResourceContents,
    },
    tool_box,
};

const IMAGE_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

const AUDIO_BASE64: &str = "UklGRiQAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQAAAAA=";

fn content_text(text: impl Into<String>) -> ContentBlock {
    TextContent::new(text.into(), None, None).into()
}

fn content_image(data: &str, mime: &str) -> ContentBlock {
    ImageContent::new(data.to_string(), mime.into(), None, None).into()
}

fn content_embedded_resource(text: &str, uri: &str, mime: &str) -> ContentBlock {
    let trc = TextResourceContents::new(text.to_string(), uri.to_string()).with_mime_type(mime);
    EmbeddedResource::new(
        EmbeddedResourceResource::TextResourceContents(trc),
        None,
        None,
    )
    .into()
}

// ---------------
// 1. test_simple_text
// ---------------
#[mcp_tool(
    name = "test_simple_text",
    description = "Returns simple text content for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SimpleTextTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl SimpleTextTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "This is a simple text response for testing.".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 2. test_image_content
// ---------------
#[mcp_tool(
    name = "test_image_content",
    description = "Returns image content (base64 PNG) for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ImageContentTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl ImageContentTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::image_content(vec![ImageContent::new(
            IMAGE_BASE64.to_string(),
            "image/png".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 3. test_audio_content
// ---------------
#[mcp_tool(
    name = "test_audio_content",
    description = "Returns audio content (base64 WAV) for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct AudioContentTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl AudioContentTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::audio_content(vec![AudioContent::new(
            AUDIO_BASE64.to_string(),
            "audio/wav".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 4. test_embedded_resource
// ---------------
#[mcp_tool(
    name = "test_embedded_resource",
    description = "Returns embedded resource content for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct EmbeddedResourceTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl EmbeddedResourceTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let block = content_embedded_resource(
            "This is an embedded resource content.",
            "test://embedded-resource",
            "text/plain",
        );
        Ok(CallToolResult {
            content: vec![block],
            is_error: None,
            meta: None,
            structured_content: None,
        })
    }
}

// ---------------
// 5. test_multiple_content_types
// ---------------
#[mcp_tool(
    name = "test_multiple_content_types",
    description = "Returns multiple content types for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct MultipleContentTypesTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl MultipleContentTypesTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult {
            content: vec![
                content_text("Multiple content types test:"),
                content_image(IMAGE_BASE64, "image/png"),
                content_embedded_resource(
                    r#"{"test":"data","value":123}"#,
                    "test://mixed-content-resource",
                    "application/json",
                ),
            ],
            is_error: None,
            meta: None,
            structured_content: None,
        })
    }
}

// ---------------
// 6. test_error_handling
// ---------------
#[mcp_tool(
    name = "test_error_handling",
    description = "Returns an error response for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ErrorHandlingTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl ErrorHandlingTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult {
            content: vec![content_text(
                "This tool intentionally returns an error for testing",
            )],
            is_error: Some(true),
            meta: None,
            structured_content: None,
        })
    }
}

// ---------------
// 7. test_tool_with_logging
// ---------------
#[mcp_tool(
    name = "test_tool_with_logging",
    description = "Sends log messages during execution for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct LoggingTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl LoggingTool {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        use rust_mcp_sdk::schema::{LoggingLevel, LoggingMessageNotificationParams};

        runtime
            .notify_log_message(LoggingMessageNotificationParams {
                level: LoggingLevel::Info,
                data: serde_json::Value::String("Tool execution started".into()),
                logger: None,
                meta: None,
            })
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .notify_log_message(LoggingMessageNotificationParams {
                level: LoggingLevel::Info,
                data: serde_json::Value::String("Tool processing data".into()),
                logger: None,
                meta: None,
            })
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .notify_log_message(LoggingMessageNotificationParams {
                level: LoggingLevel::Info,
                data: serde_json::Value::String("Tool execution completed".into()),
                logger: None,
                meta: None,
            })
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        Ok(CallToolResult::text_content(vec![TextContent::new(
            "Tool execution completed successfully".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 8. test_tool_with_progress
// ---------------
#[mcp_tool(
    name = "test_tool_with_progress",
    description = "Reports progress notifications during execution for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ProgressTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl ProgressTool {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
        progress_token: Option<rust_mcp_sdk::schema::ProgressToken>,
    ) -> Result<CallToolResult, CallToolError> {
        use rust_mcp_sdk::schema::{ProgressNotificationParams, ProgressToken};

        let token = progress_token.unwrap_or(ProgressToken::String("progress-test-1".into()));

        runtime
            .notify_progress(ProgressNotificationParams {
                progress_token: token.clone(),
                progress: 0.0,
                total: Some(100.0),
                message: None,
                meta: None,
            })
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .notify_progress(ProgressNotificationParams {
                progress_token: token.clone(),
                progress: 50.0,
                total: Some(100.0),
                message: None,
                meta: None,
            })
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .notify_progress(ProgressNotificationParams {
                progress_token: token,
                progress: 100.0,
                total: Some(100.0),
                message: None,
                meta: None,
            })
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        Ok(CallToolResult::text_content(vec![TextContent::new(
            "Progress test completed".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 9. test_sampling
// ---------------
#[mcp_tool(
    name = "test_sampling",
    description = "Requests LLM sampling from the client for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SamplingTool {
    pub prompt: String,
}

impl SamplingTool {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        use rust_mcp_sdk::schema::CreateMessageContent;

        if !runtime.client_supports_sampling().unwrap_or(false) {
            return Ok(CallToolResult {
                content: vec![content_text(
                    "Error: Client does not support sampling capability",
                )],
                is_error: Some(true),
                meta: None,
                structured_content: None,
            });
        }

        let params: rust_mcp_sdk::schema::CreateMessageRequestParams =
            serde_json::from_value(serde_json::json!({
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": self.prompt
                    }
                }],
                "maxTokens": 100
            }))
            .map_err(|e| CallToolError::from_message(format!("Failed to build params: {e}")))?;

        let response = runtime
            .request_message_creation(params)
            .await
            .map_err(|e| CallToolError::from_message(format!("Sampling failed: {e}")))?;

        let response_text = match &response.content {
            CreateMessageContent::TextContent(tc) => tc.text.clone(),
            other => format!("{:?}", other),
        };

        Ok(CallToolResult::text_content(vec![TextContent::new(
            format!("LLM response: {}", response_text),
            None,
            None,
        )]))
    }
}

// ---------------
// 10. test_elicitation
// ---------------
#[mcp_tool(
    name = "test_elicitation",
    description = "Requests user input (elicitation) from the client for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ElicitationTool {
    pub message: String,
}

impl ElicitationTool {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        use rust_mcp_sdk::schema::{
            ElicitFormSchema, ElicitRequestFormParams, ElicitRequestParams,
            PrimitiveSchemaDefinition, StringSchema,
        };
        use std::collections::BTreeMap;

        let mut properties = BTreeMap::new();
        properties.insert(
            "username".into(),
            PrimitiveSchemaDefinition::StringSchema(StringSchema::new(
                None,
                Some("User's name".into()),
                None,
                None,
                None,
                None,
            )),
        );
        properties.insert(
            "email".into(),
            PrimitiveSchemaDefinition::StringSchema(StringSchema::new(
                None,
                Some("User's email address".into()),
                None,
                None,
                None,
                None,
            )),
        );

        let schema =
            ElicitFormSchema::new(properties, vec!["username".into(), "email".into()], None);
        let params: ElicitRequestParams =
            ElicitRequestFormParams::new(self.message.clone(), schema, None, None).into();

        let response = runtime
            .request_elicitation(params)
            .await
            .map_err(|e| CallToolError::from_message(format!("Elicitation failed: {e}")))?;

        let content_text = serde_json::to_string_pretty(&response)
            .unwrap_or_else(|_| "Unable to serialize response".into());

        Ok(CallToolResult::text_content(vec![TextContent::new(
            format!(
                "User response: action={:?}, content={}",
                response.action, content_text
            ),
            None,
            None,
        )]))
    }
}

// ---------------
// 11. test_elicitation_sep1034_defaults
// ---------------
#[mcp_tool(
    name = "test_elicitation_sep1034_defaults",
    description = "Requests elicitation with default values for all primitive types (SEP-1034)."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ElicitationDefaultsTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl ElicitationDefaultsTool {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        use rust_mcp_sdk::schema::{
            BooleanSchema, ElicitFormSchema, ElicitRequestFormParams, ElicitRequestParams,
            NumberSchema, NumberSchemaType, PrimitiveSchemaDefinition, StringSchema,
            UntitledSingleSelectEnumSchema,
        };
        use std::collections::BTreeMap;

        let mut properties = BTreeMap::new();

        properties.insert(
            "name".into(),
            PrimitiveSchemaDefinition::StringSchema(StringSchema::new(
                Some("John Doe".into()),
                Some("User's full name".into()),
                None,
                None,
                None,
                None,
            )),
        );

        properties.insert(
            "age".into(),
            PrimitiveSchemaDefinition::NumberSchema(NumberSchema {
                default: Some(30.0),
                description: Some("User's age".into()),
                title: None,
                type_: NumberSchemaType::Integer,
                maximum: None,
                minimum: None,
            }),
        );

        properties.insert(
            "score".into(),
            PrimitiveSchemaDefinition::NumberSchema(NumberSchema {
                default: Some(95.5),
                description: Some("User's score".into()),
                title: None,
                type_: NumberSchemaType::Number,
                maximum: None,
                minimum: None,
            }),
        );

        properties.insert(
            "status".into(),
            PrimitiveSchemaDefinition::UntitledSingleSelectEnumSchema(
                UntitledSingleSelectEnumSchema::new(
                    vec!["active".into(), "inactive".into(), "pending".into()],
                    Some("active".into()),
                    Some("Account status".into()),
                    None,
                ),
            ),
        );

        properties.insert(
            "verified".into(),
            PrimitiveSchemaDefinition::BooleanSchema(BooleanSchema::new(
                Some(true),
                Some("Verification status".into()),
                None,
            )),
        );

        let schema = ElicitFormSchema::new(properties, vec![], None);

        let params: ElicitRequestParams = ElicitRequestFormParams::new(
            "Please provide your information".into(),
            schema,
            None,
            None,
        )
        .into();

        let response = runtime
            .request_elicitation(params)
            .await
            .map_err(|e| CallToolError::from_message(format!("Elicitation failed: {e}")))?;

        let content_text = serde_json::to_string_pretty(&response)
            .unwrap_or_else(|_| "Unable to serialize response".into());

        Ok(CallToolResult::text_content(vec![TextContent::new(
            format!("Elicitation completed: content={}", content_text),
            None,
            None,
        )]))
    }
}

// ---------------
// 12. test_elicitation_sep1330_enums
// ---------------
#[mcp_tool(
    name = "test_elicitation_sep1330_enums",
    description = "Requests elicitation with all enum variants (SEP-1330)."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct ElicitationEnumsTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl ElicitationEnumsTool {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        use rust_mcp_sdk::schema::{
            ElicitFormSchema, ElicitRequestFormParams, ElicitRequestParams, LegacyTitledEnumSchema,
            PrimitiveSchemaDefinition, TitledMultiSelectEnumSchema,
            TitledMultiSelectEnumSchemaItems, TitledMultiSelectEnumSchemaItemsAnyOfItem,
            TitledSingleSelectEnumSchema, TitledSingleSelectEnumSchemaOneOfItem,
            UntitledMultiSelectEnumSchema, UntitledMultiSelectEnumSchemaItems,
            UntitledSingleSelectEnumSchema,
        };
        use std::collections::BTreeMap;

        let mut properties = BTreeMap::new();

        properties.insert(
            "untitledSingle".into(),
            PrimitiveSchemaDefinition::UntitledSingleSelectEnumSchema(
                UntitledSingleSelectEnumSchema::new(
                    vec!["option1".into(), "option2".into(), "option3".into()],
                    None,
                    Some("Untitled single-select".into()),
                    None,
                ),
            ),
        );

        properties.insert(
            "titledSingle".into(),
            PrimitiveSchemaDefinition::TitledSingleSelectEnumSchema(
                TitledSingleSelectEnumSchema::new(
                    vec![
                        TitledSingleSelectEnumSchemaOneOfItem {
                            const_: "value1".into(),
                            title: "First Option".into(),
                        },
                        TitledSingleSelectEnumSchemaOneOfItem {
                            const_: "value2".into(),
                            title: "Second Option".into(),
                        },
                        TitledSingleSelectEnumSchemaOneOfItem {
                            const_: "value3".into(),
                            title: "Third Option".into(),
                        },
                    ],
                    None,
                    Some("Titled single-select".into()),
                    None,
                ),
            ),
        );

        properties.insert(
            "legacyEnum".into(),
            PrimitiveSchemaDefinition::LegacyTitledEnumSchema(LegacyTitledEnumSchema::new(
                vec!["opt1".into(), "opt2".into(), "opt3".into()],
                vec![
                    "Option One".into(),
                    "Option Two".into(),
                    "Option Three".into(),
                ],
                None,
                Some("Legacy titled enum".into()),
                None,
            )),
        );

        properties.insert(
            "untitledMulti".into(),
            PrimitiveSchemaDefinition::UntitledMultiSelectEnumSchema(
                UntitledMultiSelectEnumSchema::new(
                    vec![],
                    UntitledMultiSelectEnumSchemaItems::new(vec![
                        "option1".into(),
                        "option2".into(),
                        "option3".into(),
                    ]),
                    Some("Untitled multi-select".into()),
                    None,
                    None,
                    None,
                ),
            ),
        );

        properties.insert(
            "titledMulti".into(),
            PrimitiveSchemaDefinition::TitledMultiSelectEnumSchema(
                TitledMultiSelectEnumSchema::new(
                    vec![],
                    TitledMultiSelectEnumSchemaItems {
                        any_of: vec![
                            TitledMultiSelectEnumSchemaItemsAnyOfItem {
                                const_: "value1".into(),
                                title: "First Choice".into(),
                            },
                            TitledMultiSelectEnumSchemaItemsAnyOfItem {
                                const_: "value2".into(),
                                title: "Second Choice".into(),
                            },
                            TitledMultiSelectEnumSchemaItemsAnyOfItem {
                                const_: "value3".into(),
                                title: "Third Choice".into(),
                            },
                        ],
                    },
                    Some("Titled multi-select".into()),
                    None,
                    None,
                    None,
                ),
            ),
        );

        let schema = ElicitFormSchema::new(properties, vec![], None);

        let params: ElicitRequestParams =
            ElicitRequestFormParams::new("Select from enum options".into(), schema, None, None)
                .into();

        let response = runtime
            .request_elicitation(params)
            .await
            .map_err(|e| CallToolError::from_message(format!("Elicitation failed: {e}")))?;

        let content_text = serde_json::to_string_pretty(&response)
            .unwrap_or_else(|_| "Unable to serialize response".into());

        Ok(CallToolResult::text_content(vec![TextContent::new(
            format!("Elicitation completed: content={}", content_text),
            None,
            None,
        )]))
    }
}

// ---------------
// Tool box
// ---------------
tool_box!(
    ConformanceTools,
    [
        SimpleTextTool,
        ImageContentTool,
        AudioContentTool,
        EmbeddedResourceTool,
        MultipleContentTypesTool,
        ErrorHandlingTool,
        LoggingTool,
        ProgressTool,
        SamplingTool,
        ElicitationTool,
        ElicitationDefaultsTool,
        ElicitationEnumsTool,
    ]
);
