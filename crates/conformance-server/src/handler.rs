use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CompleteRequestParams, CompleteRequestRef, CompleteResult,
    CompleteResultCompletion, GetPromptRequestParams, ListPromptsResult,
    ListPromptsResultCacheScope, ListResourceTemplatesResult,
    ListResourceTemplatesResultCacheScope, ListResourcesResult, ListResourcesResultCacheScope,
    ListToolsResult, ListToolsResultCacheScope, PaginatedRequestParams, ReadResourceRequestParams,
    RpcError, ServerResult,
};
use rust_mcp_sdk::{McpServer, RequestContext, RequiredClientCapability};
use std::sync::Arc;

use crate::{
    prompts,
    resources::{
        EmbeddedTestResource, StaticBinaryResource, StaticTextResource, TemplateDataResource,
        WatchedResource,
    },
    tools::ConformanceTools,
};

/// Inject the JSON Schema 2020-12 keywords required by the
/// `json-schema-2020-12` conformance scenario into a tool's input schema.
/// The `#[mcp_tool]` macro generates a basic JSON Schema; the extra
/// keywords (`$schema`, `$defs`, `allOf`/`anyOf`, `if`/`then`/`else`,
/// `additionalProperties`) are prescribed by the conformance specification
/// and must be present verbatim in the `tools/list` response.
fn inject_json_schema_2020_12_keywords(tool: &mut rust_mcp_sdk::schema::Tool) {
    let extra_map = tool
        .input_schema
        .extra
        .get_or_insert_with(serde_json::Map::new);

    use serde_json::json;
    let keywords: serde_json::Map<String, serde_json::Value> = serde_json::from_value(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "additionalProperties": false,
        "$defs": {
            "address": {
                "$anchor": "Address",
                "type": "object",
                "properties": {
                    "street": { "type": "string" },
                    "city": { "type": "string" }
                }
            }
        },
        "allOf": [
            {
                "type": "object",
                "properties": { "name": { "type": "string" } }
            },
            {
                "anyOf": [
                    {
                        "type": "object",
                        "properties": { "address": { "$ref": "#/$defs/address" } }
                    },
                    {
                        "type": "object",
                        "properties": { "alt_address": { "$ref": "#/$defs/address" } }
                    }
                ]
            }
        ],
        "if": {
            "type": "object",
            "properties": { "kind": { "const": "premium" } }
        },
        "then": {
            "type": "object",
            "properties": { "fee": { "type": "number", "minimum": 0 } }
        },
        "else": {
            "type": "object",
            "properties": { "note": { "type": "string" } }
        }
    }))
    .expect("static JSON Schema 2020-12 keywords");
    extra_map.extend(keywords);
    // JSON Schema 2020-12's `additionalProperties` at the root conflicts with
    // the generated `properties` — the latter is a child, not excluded — so
    // keep both. The `allOf` etc. coexist with the generated `properties`.
}

pub struct ConformanceHandler;

#[async_trait]
impl ServerHandler for ConformanceHandler {
    fn required_capabilities_for_tool_call(
        &self,
        tool_name: &str,
    ) -> Vec<RequiredClientCapability> {
        if tool_name == "test_missing_capability" {
            vec![RequiredClientCapability::Sampling]
        } else {
            Vec::new()
        }
    }

    fn tool_header_annotations(
        &self,
        tool_name: &str,
    ) -> Vec<rust_mcp_sdk::tool_param_headers::ToolParamHeader> {
        if tool_name == "test_custom_header_tool" {
            rust_mcp_sdk::tool_param_headers::annotations_for_tool(
                &crate::tools::TestCustomHeaderTool::tool(),
            )
            .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        let mut tools = ConformanceTools::tools();
        tools.extend(crate::mrtr_tools::tools());

        // Inject SEP-1613 / SEP-2106 JSON Schema 2020-12 keywords into the
        // fixture tool's input schema without hard-coding the entire Tool.
        if let Some(tool) = tools
            .iter_mut()
            .find(|t| t.name == "json_schema_2020_12_tool")
        {
            inject_json_schema_2020_12_keywords(tool);
        }

        Ok(ListToolsResult {
            tools,
            cache_scope: ListToolsResultCacheScope::Public,
            meta: None,
            next_cursor: None,
            result_type: "complete".to_string(),
            ttl_ms: 0,
        })
    }

    async fn handle_call_tool_request_mrtr(
        &self,
        params: CallToolRequestParams,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> Result<ServerResult, RpcError> {
        if let Some(result) = crate::mrtr_tools::handle_tool_call(&params) {
            return result;
        }
        if params.name == "test_streaming_elicitation" {
            return Ok(ServerResult::InputRequiredResult(
                rust_mcp_sdk::schema::InputRequiredResult {
                    input_requests: Some(crate::mrtr_tools::elicit_input_requests(
                        "confirm",
                        "Please confirm to continue",
                        "ok",
                        "boolean",
                    )),
                    meta: None,
                    request_state: None,
                    result_type: "input_required".to_string(),
                },
            ));
        }
        if params.name == "test_sampling" {
            return Ok(ServerResult::InputRequiredResult(
                rust_mcp_sdk::schema::InputRequiredResult {
                    input_requests: Some(crate::mrtr_tools::input_requests(vec![(
                        "sampling",
                        crate::mrtr_tools::sampling_request("What is 2+2?", 100),
                    )])),
                    meta: None,
                    request_state: None,
                    result_type: "input_required".to_string(),
                },
            ));
        }
        if params.name == "test_elicitation"
            || params.name == "test_elicitation_defaults"
            || params.name == "test_elicitation_enums"
        {
            return Ok(ServerResult::InputRequiredResult(
                rust_mcp_sdk::schema::InputRequiredResult {
                    input_requests: Some(crate::mrtr_tools::elicit_input_requests(
                        "info",
                        "Please provide the requested information",
                        "username",
                        "string",
                    )),
                    meta: None,
                    request_state: None,
                    result_type: "input_required".to_string(),
                },
            ));
        }
        match self
            .handle_call_tool_request(params, context, runtime)
            .await
        {
            Ok(result) => Ok(result),
            Err(tool_error) => {
                let result: rust_mcp_sdk::schema::CallToolResult = tool_error.into();
                Ok(result.into())
            }
        }
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> Result<ServerResult, CallToolError> {
        let progress_token = params.meta.progress_token.clone();
        let tool_params: ConformanceTools =
            ConformanceTools::try_from(params).map_err(CallToolError::new)?;

        match tool_params {
            ConformanceTools::TestSimpleText(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestImageContent(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestAudioContent(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestEmbeddedResource(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestMultipleContentTypes(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestErrorHandling(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestToolWithLogging(t) => {
                t.call_tool(&runtime).await.map(|r| r.into())
            }
            ConformanceTools::TestToolWithProgress(t) => t
                .call_tool(&runtime, progress_token)
                .await
                .map(|r| r.into()),
            ConformanceTools::TestSampling(t) => t.call_tool(&runtime).await.map(|r| r.into()),
            ConformanceTools::TestElicitation(t) => t.call_tool(&runtime).await.map(|r| r.into()),
            ConformanceTools::TestElicitationDefaults(t) => {
                t.call_tool(&runtime).await.map(|r| r.into())
            }
            ConformanceTools::TestElicitationEnums(t) => {
                t.call_tool(&runtime).await.map(|r| r.into())
            }
            ConformanceTools::TestMissingCapability(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestCustomHeaderTool(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestJsonSchema202012Tool(t) => t.call_tool().map(|r| r.into()),
            ConformanceTools::TestTriggerToolChange(t) => {
                t.call_tool(&runtime).await.map(|r| r.into())
            }
            ConformanceTools::TestTriggerPromptChange(t) => {
                t.call_tool(&runtime).await.map(|r| r.into())
            }
            ConformanceTools::TestLoggingTool(t) => t.call_tool(&runtime).await.map(|r| r.into()),
            // Intercepted by `handle_call_tool_request_mrtr`.
            ConformanceTools::TestStreamingElicitation(_) => {
                unreachable!(
                    "test_streaming_elicitation dispatched via handle_call_tool_request_mrtr"
                )
            }
        }
    }

    async fn handle_list_resources_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListResourcesResult, RpcError> {
        Ok(ListResourcesResult {
            resources: vec![
                StaticTextResource::resource(),
                StaticBinaryResource::resource(),
                EmbeddedTestResource::resource(),
                WatchedResource::resource(),
            ],
            cache_scope: ListResourcesResultCacheScope::Public,
            meta: None,
            next_cursor: None,
            result_type: "complete".to_string(),
            ttl_ms: 0,
        })
    }

    async fn handle_list_resource_templates_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListResourceTemplatesResult, RpcError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![TemplateDataResource::resource_template()],
            cache_scope: ListResourceTemplatesResultCacheScope::Public,
            meta: None,
            next_cursor: None,
            result_type: "complete".to_string(),
            ttl_ms: 0,
        })
    }

    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ServerResult, RpcError> {
        let uri = &params.uri;

        if uri == StaticTextResource::resource_uri() {
            return StaticTextResource::get_resource().await.map(|r| r.into());
        }
        if uri == StaticBinaryResource::resource_uri() {
            return StaticBinaryResource::get_resource().await.map(|r| r.into());
        }
        if uri == EmbeddedTestResource::resource_uri() {
            return EmbeddedTestResource::get_resource().await.map(|r| r.into());
        }
        if uri == WatchedResource::resource_uri() {
            return WatchedResource::get_resource().await.map(|r| r.into());
        }
        if TemplateDataResource::matches_url(uri) {
            return TemplateDataResource::get_resource(uri)
                .await
                .map(|r| r.into());
        }

        Err(RpcError::invalid_request()
            .with_message(format!("No resource found for uri '{}'.", uri)))
    }

    async fn handle_list_prompts_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListPromptsResult, RpcError> {
        let mut prompts = prompts::all_prompts();
        prompts.push(crate::mrtr_tools::prompt());
        Ok(ListPromptsResult {
            prompts,
            cache_scope: ListPromptsResultCacheScope::Public,
            meta: None,
            next_cursor: None,
            result_type: "complete".to_string(),
            ttl_ms: 0,
        })
    }

    async fn handle_get_prompt_request(
        &self,
        params: GetPromptRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ServerResult, RpcError> {
        if let Some(result) = crate::mrtr_tools::handle_prompt_get(&params.name, &params) {
            return result;
        }
        match params.name.as_str() {
            "test_simple_prompt" => prompts::TestSimplePrompt::get_prompt().map(|r| r.into()),
            "test_prompt_with_arguments" => {
                let args = params.arguments.as_ref().ok_or_else(|| {
                    RpcError::invalid_params().with_message(
                        "Arguments required for test_prompt_with_arguments".to_string(),
                    )
                })?;
                let arg1 = args.get("arg1").map(String::as_str).unwrap_or("default1");
                let arg2 = args.get("arg2").map(String::as_str).unwrap_or("default2");
                prompts::TestPromptWithArguments::get_prompt(arg1, arg2).map(|r| r.into())
            }
            "test_prompt_with_embedded_resource" => {
                let args = params.arguments.as_ref().ok_or_else(|| {
                    RpcError::invalid_params().with_message(
                        "Arguments required for test_prompt_with_embedded_resource".to_string(),
                    )
                })?;
                let uri = args
                    .get("resourceUri")
                    .map(String::as_str)
                    .unwrap_or("test://example-resource");
                prompts::TestPromptWithEmbeddedResource::get_prompt(uri).map(|r| r.into())
            }
            "test_prompt_with_image" => {
                prompts::TestPromptWithImage::get_prompt().map(|r| r.into())
            }
            _ => Err(RpcError::invalid_params()
                .with_message(format!("Unknown prompt: '{}'", params.name))),
        }
    }

    async fn handle_complete_request(
        &self,
        params: CompleteRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<CompleteResult, RpcError> {
        if matches!(&params.ref_, CompleteRequestRef::PromptReference(pr) if pr.name == "test_prompt_with_arguments")
            && params.argument.name == "arg1"
        {
            Ok(CompleteResult {
                completion: CompleteResultCompletion {
                    values: vec!["paris".into(), "park".into(), "party".into()],
                    has_more: Some(false),
                    total: Some(3),
                },
                meta: None,
                result_type: "complete".to_string(),
            })
        } else {
            Err(RpcError::method_not_found().with_message(format!(
                "No completion handler for '{}'",
                params.argument.name
            )))
        }
    }
}
