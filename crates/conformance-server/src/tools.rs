use rust_mcp_macros::JsonSchema;
use rust_mcp_sdk::{
    macros::mcp_tool,
    schema::{
        schema_utils::CallToolError, AudioContent, CallToolResult, ContentBlock,
        EmbeddedResourceResource, ImageContent, TextContent, TextResourceContents,
    },
    tool_box,
};

const IMAGE_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

const AUDIO_BASE64: &str = "UklGRiQAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQAAAAA=";

// ---------------
// 1. test_simple_text
// ---------------
#[mcp_tool(
    name = "test_simple_text",
    description = "Returns simple text content for conformance testing."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestSimpleText {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestSimpleText {
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
pub struct TestImageContent {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestImageContent {
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
pub struct TestAudioContent {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestAudioContent {
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
pub struct TestEmbeddedResource {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestEmbeddedResource {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let block =
            ContentBlock::embedded_resource(EmbeddedResourceResource::TextResourceContents(
                TextResourceContents::new(
                    "This is an embedded resource content.",
                    "test://embedded-resource",
                )
                .with_mime_type("text/plain".to_string()),
            ));
        Ok(CallToolResult {
            content: vec![block],
            is_error: None,
            meta: None,
            structured_content: None,
            result_type: "complete".to_string(),
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
pub struct TestMultipleContentTypes {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestMultipleContentTypes {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult {
            content: vec![
                ContentBlock::text_content("Multiple content types test:".to_string()),
                ContentBlock::image_content(IMAGE_BASE64.to_string(), "image/png".to_string()),
                ContentBlock::embedded_resource(EmbeddedResourceResource::TextResourceContents(
                    TextResourceContents::new(
                        r#"{"test":"data","value":123}"#,
                        "test://mixed-content-resource",
                    )
                    .with_mime_type("application/json".to_string()),
                )),
            ],
            is_error: None,
            meta: None,
            structured_content: None,
            result_type: "complete".to_string(),
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
pub struct TestErrorHandling {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestErrorHandling {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult {
            content: vec![ContentBlock::text_content(
                "This tool intentionally returns an error for testing".to_string(),
            )],
            is_error: Some(true),
            meta: None,
            structured_content: None,
            result_type: "complete".to_string(),
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
pub struct TestToolWithLogging {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestToolWithLogging {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        runtime
            .log_info("Tool execution started".into())
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .log_info("Tool processing data".into())
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .log_info("Tool execution completed".into())
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
pub struct TestToolWithProgress {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestToolWithProgress {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
        progress_token: Option<rust_mcp_sdk::schema::ProgressToken>,
    ) -> Result<CallToolResult, CallToolError> {
        use rust_mcp_sdk::schema::ProgressToken;

        let token = Some(progress_token.unwrap_or(ProgressToken::String("progress-test-1".into())));

        runtime
            .report_progress(token.clone(), 0.0, Some(100.0), None)
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .report_progress(token.clone(), 50.0, Some(100.0), None)
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        runtime
            .report_progress(token, 100.0, Some(100.0), None)
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
pub struct TestSampling {
    pub prompt: String,
}

impl TestSampling {
    pub async fn call_tool(
        &self,
        _runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        // 2026-07-28: standalone sampling removed; intercepted by handle_call_tool_request_mrtr.
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "test_sampling: handled via MRTR InputRequiredResult".into(),
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
pub struct TestElicitation {
    pub message: String,
}

impl TestElicitation {
    pub async fn call_tool(
        &self,
        _runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "test_elicitation: handled via MRTR InputRequiredResult".into(),
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
pub struct TestElicitationDefaults {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestElicitationDefaults {
    pub async fn call_tool(
        &self,
        _runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        // 2026-07-28: standalone elicitation removed; intercepted by handle_call_tool_request_mrtr.
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "elicitation tool: handled via MRTR InputRequiredResult".into(),
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
pub struct TestElicitationEnums {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestElicitationEnums {
    pub async fn call_tool(
        &self,
        _runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        // 2026-07-28: standalone elicitation removed; intercepted by handle_call_tool_request_mrtr.
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "elicitation tool: handled via MRTR InputRequiredResult".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 13. test_missing_capability
// ---------------
/// Diagnostic tool used by the `server-stateless` conformance scenario: it
/// declares a `sampling` client-capability requirement (see
/// `ConformanceHandler::required_capabilities_for_tool_call`), so a client that
/// did not declare `sampling` in its `_meta` client capabilities must be
/// rejected with `MissingRequiredClientCapabilityError` (-32021).
#[mcp_tool(
    name = "test_missing_capability",
    description = "Requires the client to declare the sampling capability; the server rejects the call with -32021 when it is not declared."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestMissingCapability {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestMissingCapability {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "Client declared the required sampling capability.".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 14. test_custom_header_tool (SEP-2243 server-side)
// ---------------
#[mcp_tool(
    name = "test_custom_header_tool",
    description = "A tool with an x-mcp-header annotation used by the SEP-2243 server-side custom-header validation scenario."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestCustomHeaderTool {
    #[json_schema(x_mcp_header = "Region")]
    pub region: String,
}

impl TestCustomHeaderTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec![TextContent::new(
            format!("region: {}", self.region),
            None,
            None,
        )]))
    }
}

// ---------------
// 15. json_schema_2020_12_tool (SEP-1613 / SEP-2106)
// ---------------
#[mcp_tool(
    name = "json_schema_2020_12_tool",
    description = "SEP-1613/SEP-2106 fixture: JSON Schema 2020-12 keyword preservation test."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestJsonSchema202012Tool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestJsonSchema202012Tool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "json-schema-2020-12 check done".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 16. test_trigger_tool_change (list-changed diagnostic)
// ---------------
#[mcp_tool(
    name = "test_trigger_tool_change",
    description = "SEP-2575 diagnostic: emits a tools/list_changed notification."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestTriggerToolChange {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestTriggerToolChange {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        runtime
            .notify_tool_list_changed(None)
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "Tool list change triggered.".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 17. test_trigger_prompt_change
// ---------------
#[mcp_tool(
    name = "test_trigger_prompt_change",
    description = "SEP-2575 diagnostic: emits a prompts/list_changed notification."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestTriggerPromptChange {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestTriggerPromptChange {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        runtime
            .notify_prompt_list_changed(None)
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "Prompt list change triggered.".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 18. test_logging_tool (SEP-2575 log-level diagnostic)
// ---------------
#[mcp_tool(
    name = "test_logging_tool",
    description = "SEP-2575 diagnostic: emits log notifications; the runtime gates delivery on _meta.logLevel."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestLoggingTool {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestLoggingTool {
    pub async fn call_tool(
        &self,
        runtime: &std::sync::Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        runtime
            .log_debug("Tool execution started".into())
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .log_info("Tool processing data".into())
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .log_warn("Tool execution completed".into())
            .await
            .map_err(|e| CallToolError::from_message(format!("{e}")))?;
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "Logging test completed".into(),
            None,
            None,
        )]))
    }
}

// ---------------
// 19. test_streaming_elicitation (stream-integrity diagnostic)
// ---------------
#[mcp_tool(
    name = "test_streaming_elicitation",
    description = "SEP-2575 diagnostic: returns InputRequiredResult with elicitation — no independent requests on stream."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestStreamingElicitation {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

impl TestStreamingElicitation {
    #[allow(dead_code)]
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        // This is intentionally a no-op with an empty result — the MRTR
        // handler intercepts it at the `handle_call_tool_request_mrtr` level
        // and returns an InputRequiredResult. Direct calls via the tool_box
        // dispatch (which never happen for this tool because the MRTR handler
        // matches first) are handled here for exhaustiveness.
        Ok(CallToolResult::text_content(vec![TextContent::new(
            "streaming-elicitation ok".into(),
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
        TestSimpleText,
        TestImageContent,
        TestAudioContent,
        TestEmbeddedResource,
        TestMultipleContentTypes,
        TestErrorHandling,
        TestToolWithLogging,
        TestToolWithProgress,
        TestSampling,
        TestElicitation,
        TestElicitationDefaults,
        TestElicitationEnums,
        TestMissingCapability,
        TestCustomHeaderTool,
        TestJsonSchema202012Tool,
        TestTriggerToolChange,
        TestTriggerPromptChange,
        TestLoggingTool,
        TestStreamingElicitation,
    ]
);
