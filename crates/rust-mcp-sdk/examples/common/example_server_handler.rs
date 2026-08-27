use super::tools::GreetingTools;
use crate::common::resources::{BlobResource, PlainTextResource, PokemonImageResource};
use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::ServerHandler,
    schema::{
        schema_utils::CallToolError, CallToolRequestParams, CompleteRequestParams, CompleteResult,
        ListResourceTemplatesResult, ListResourceTemplatesResultCacheScope, ListResourcesResult,
        ListResourcesResultCacheScope, ListToolsResult, ListToolsResultCacheScope,
        PaginatedRequestParams, ReadResourceRequestParams, RpcError, ServerResult,
    },
    McpServer, RequestContext,
};
use std::sync::Arc;

pub struct ExampleServerHandler;

#[async_trait]
#[allow(unused)]
impl ServerHandler for ExampleServerHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            cache_scope: ListToolsResultCacheScope::Private,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            meta: None,
            next_cursor: None,
            tools: GreetingTools::tools(),
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, CallToolError> {
        let tool_params: GreetingTools =
            GreetingTools::try_from(params).map_err(CallToolError::new)?;

        match tool_params {
            GreetingTools::SayHelloTool(say_hello_tool) => {
                say_hello_tool.call_tool().map(|r| r.into())
            }
            GreetingTools::SayGoodbyeTool(say_goodbye_tool) => {
                say_goodbye_tool.call_tool().map(|r| r.into())
            }
        }
    }

    async fn handle_list_resources_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Ok(ListResourcesResult {
            cache_scope: ListResourcesResultCacheScope::Private,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            meta: None,
            next_cursor: None,
            resources: vec![PlainTextResource::resource(), BlobResource::resource()],
        })
    }

    async fn handle_list_resource_templates_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, RpcError> {
        Ok(ListResourceTemplatesResult {
            cache_scope: ListResourceTemplatesResultCacheScope::Private,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            meta: None,
            next_cursor: None,
            resource_templates: vec![PokemonImageResource::resource_template()],
        })
    }

    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        if PlainTextResource::resource_uri().starts_with(&params.uri) {
            return PlainTextResource::get_resource()
                .await
                .map(ServerResult::from);
        }
        if BlobResource::resource_uri().starts_with(&params.uri) {
            return BlobResource::get_resource().await.map(ServerResult::from);
        }
        if PokemonImageResource::matches_url(&params.uri) {
            return PokemonImageResource::get_resource(&params.uri)
                .await
                .map(ServerResult::from);
        }
        Err(RpcError::invalid_request()
            .with_message(format!("No resource was found for '{}'.", params.uri)))
    }

    async fn handle_complete_request(
        &self,
        params: CompleteRequestParams,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CompleteResult, RpcError> {
        if params.argument.name.eq("pokemon-id") {
            Ok(CompleteResult {
                completion: PokemonImageResource::completion(&params.argument.value),
                result_type: "complete".to_string(),
                meta: None,
            })
        } else {
            Err(RpcError::method_not_found().with_message(format!(
                "No handler is implemented for '{}'.",
                params.argument.name,
            )))
        }
    }
}
