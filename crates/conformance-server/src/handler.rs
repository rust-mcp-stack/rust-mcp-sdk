use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, CompleteRequestParams, CompleteRequestRef, CompleteResult,
    CompleteResultCompletion, GetPromptRequestParams, GetPromptResult, ListPromptsResult,
    ListResourceTemplatesResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResult, RpcError, SetLevelRequestParams,
    SubscribeRequestParams, UnsubscribeRequestParams,
};
use rust_mcp_sdk::McpServer;
use std::sync::Arc;

use crate::{
    prompts,
    resources::{
        EmbeddedTestResource, StaticBinaryResource, StaticTextResource, TemplateDataResource,
        WatchedResource,
    },
    tools::ConformanceTools,
};

pub struct ConformanceHandler;

#[async_trait]
impl ServerHandler for ConformanceHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: ConformanceTools::tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        let progress_token = params
            .meta
            .as_ref()
            .and_then(|m| m.progress_token.clone());
        let tool_params: ConformanceTools =
            ConformanceTools::try_from(params).map_err(CallToolError::new)?;

        match tool_params {
            ConformanceTools::TestSimpleText(t) => t.call_tool(),
            ConformanceTools::TestImageContent(t) => t.call_tool(),
            ConformanceTools::TestAudioContent(t) => t.call_tool(),
            ConformanceTools::TestEmbeddedResource(t) => t.call_tool(),
            ConformanceTools::TestMultipleContentTypes(t) => t.call_tool(),
            ConformanceTools::TestErrorHandling(t) => t.call_tool(),
            ConformanceTools::TestToolWithLogging(t) => t.call_tool(&runtime).await,
            ConformanceTools::TestToolWithProgress(t) => t.call_tool(&runtime, progress_token).await,
            ConformanceTools::TestSampling(t) => t.call_tool(&runtime).await,
            ConformanceTools::TestElicitation(t) => t.call_tool(&runtime).await,
            ConformanceTools::TestElicitationDefaults(t) => t.call_tool(&runtime).await,
            ConformanceTools::TestElicitationEnums(t) => t.call_tool(&runtime).await,
        }
    }

    async fn handle_list_resources_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListResourcesResult, RpcError> {
        Ok(ListResourcesResult {
            resources: vec![
                StaticTextResource::resource(),
                StaticBinaryResource::resource(),
                EmbeddedTestResource::resource(),
                WatchedResource::resource(),
            ],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_list_resource_templates_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListResourceTemplatesResult, RpcError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![TemplateDataResource::resource_template()],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ReadResourceResult, RpcError> {
        let uri = &params.uri;

        if uri == StaticTextResource::resource_uri() {
            return StaticTextResource::get_resource().await;
        }
        if uri == StaticBinaryResource::resource_uri() {
            return StaticBinaryResource::get_resource().await;
        }
        if uri == EmbeddedTestResource::resource_uri() {
            return EmbeddedTestResource::get_resource().await;
        }
        if uri == WatchedResource::resource_uri() {
            return WatchedResource::get_resource().await;
        }
        if TemplateDataResource::matches_url(uri) {
            return TemplateDataResource::get_resource(uri).await;
        }

        Err(RpcError::invalid_request()
            .with_message(format!("No resource found for uri '{}'.", uri)))
    }

    async fn handle_list_prompts_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListPromptsResult, RpcError> {
        Ok(ListPromptsResult {
            prompts: prompts::all_prompts(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_get_prompt_request(
        &self,
        params: GetPromptRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<GetPromptResult, RpcError> {
        match params.name.as_str() {
            "test_simple_prompt" => prompts::TestSimplePrompt::get_prompt(),
            "test_prompt_with_arguments" => {
                let args = params.arguments.as_ref().ok_or_else(|| {
                    RpcError::invalid_params()
                        .with_message("Arguments required for test_prompt_with_arguments".to_string())
                })?;
                let arg1 = args.get("arg1").map(String::as_str).unwrap_or("default1");
                let arg2 = args.get("arg2").map(String::as_str).unwrap_or("default2");
                prompts::TestPromptWithArguments::get_prompt(arg1, arg2)
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
                prompts::TestPromptWithEmbeddedResource::get_prompt(uri)
            }
            "test_prompt_with_image" => prompts::TestPromptWithImage::get_prompt(),
            _ => Err(RpcError::invalid_params()
                .with_message(format!("Unknown prompt: '{}'", params.name))),
        }
    }

    async fn handle_complete_request(
        &self,
        params: CompleteRequestParams,
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
            })
        } else {
            Err(RpcError::method_not_found()
                .with_message(format!("No completion handler for '{}'", params.argument.name)))
        }
    }

    async fn handle_set_level_request(
        &self,
        _params: SetLevelRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<rust_mcp_sdk::schema::Result, RpcError> {
        Ok(rust_mcp_sdk::schema::Result::default())
    }

    async fn handle_subscribe_request(
        &self,
        params: SubscribeRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<rust_mcp_sdk::schema::Result, RpcError> {
        tracing::info!("Subscribed to resource: {}", params.uri);
        Ok(rust_mcp_sdk::schema::Result::default())
    }

    async fn handle_unsubscribe_request(
        &self,
        params: UnsubscribeRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<rust_mcp_sdk::schema::Result, RpcError> {
        tracing::info!("Unsubscribed from resource: {}", params.uri);
        Ok(rust_mcp_sdk::schema::Result::default())
    }
}
