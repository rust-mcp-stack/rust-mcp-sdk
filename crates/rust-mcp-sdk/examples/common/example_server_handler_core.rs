use crate::common::resources::{BlobResource, PlainTextResource, PokemonImageResource};

use super::tools::GreetingTools;
use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::ServerHandlerCore,
    schema::{
        schema_utils::CallToolError, CompleteResult, DiscoverResult, DiscoverResultCacheScope,
        ListResourceTemplatesResult, ListResourceTemplatesResultCacheScope, ListResourcesResult,
        ListResourcesResultCacheScope, ListToolsResult, ListToolsResultCacheScope,
        NotificationFromClient, RequestFromClient, RpcError, ServerResult,
    },
    McpServer, RequestContext,
};
use std::sync::Arc;

pub struct ExampleServerHandlerCore;

#[async_trait]
#[allow(unused)]
impl ServerHandlerCore for ExampleServerHandlerCore {
    async fn handle_request(
        &self,
        request: RequestFromClient,
        _context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        let method_name = request.method().to_owned();
        match request {
            RequestFromClient::DiscoverRequest(_params) => {
                let details = runtime.server_details();
                Ok(ServerResult::DiscoverResult(DiscoverResult {
                    capabilities: details.capabilities.clone(),
                    instructions: details.instructions.clone(),
                    meta: details.meta.clone(),
                    result_type: "complete".to_string(),
                    cache_scope: DiscoverResultCacheScope::Private,
                    ttl_ms: 0,
                    supported_versions: vec![
                        rust_mcp_sdk::schema::ProtocolVersion::V2026_07_28.to_string()
                    ],
                }))
            }

            RequestFromClient::ListToolsRequest(_params) => Ok(ListToolsResult {
                cache_scope: ListToolsResultCacheScope::Private,
                result_type: "complete".to_string(),
                ttl_ms: 0,
                meta: None,
                next_cursor: None,
                tools: GreetingTools::tools(),
            }
            .into()),

            RequestFromClient::CallToolRequest(params) => {
                let tool_name = params.name.to_string();
                let tool_params = GreetingTools::try_from(params)
                    .map_err(|_| CallToolError::unknown_tool(tool_name.clone()))?;
                let result = match tool_params {
                    GreetingTools::SayHelloTool(say_hello_tool) => say_hello_tool
                        .call_tool()
                        .map_err(|err| RpcError::internal_error().with_message(err.to_string()))?,
                    GreetingTools::SayGoodbyeTool(say_goodbye_tool) => say_goodbye_tool
                        .call_tool()
                        .map_err(|err| RpcError::internal_error().with_message(err.to_string()))?,
                };
                Ok(result.into())
            }

            RequestFromClient::ListResourcesRequest(_params) => Ok(ListResourcesResult {
                cache_scope: ListResourcesResultCacheScope::Private,
                result_type: "complete".to_string(),
                ttl_ms: 0,
                meta: None,
                next_cursor: None,
                resources: vec![PlainTextResource::resource(), BlobResource::resource()],
            }
            .into()),

            RequestFromClient::ListResourceTemplatesRequest(_params) => {
                Ok(ListResourceTemplatesResult {
                    cache_scope: ListResourceTemplatesResultCacheScope::Private,
                    result_type: "complete".to_string(),
                    ttl_ms: 0,
                    meta: None,
                    next_cursor: None,
                    resource_templates: vec![PokemonImageResource::resource_template()],
                }
                .into())
            }

            RequestFromClient::ReadResourceRequest(params) => {
                if PlainTextResource::resource_uri().starts_with(&params.uri) {
                    return PlainTextResource::get_resource().await.map(|r| r.into());
                }
                if BlobResource::resource_uri().starts_with(&params.uri) {
                    return BlobResource::get_resource().await.map(|r| r.into());
                }
                if PokemonImageResource::matches_url(&params.uri) {
                    return PokemonImageResource::get_resource(&params.uri)
                        .await
                        .map(|r| r.into());
                }
                Err(RpcError::invalid_request()
                    .with_message(format!("No resource was found for '{}'.", params.uri)))
            }

            RequestFromClient::CompleteRequest(params) => {
                if params.argument.name.eq("pokemon-id") {
                    Ok(CompleteResult {
                        completion: PokemonImageResource::completion(&params.argument.value),
                        result_type: "complete".to_string(),
                        meta: None,
                    }
                    .into())
                } else {
                    Err(RpcError::method_not_found().with_message(format!(
                        "No handler is implemented for '{}'.",
                        params.argument.name,
                    )))
                }
            }

            _ => Err(RpcError::method_not_found()
                .with_message(format!("No handler is implemented for '{method_name}'.",))),
        }
    }

    async fn handle_notification(
        &self,
        _notification: NotificationFromClient,
        _: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_error(
        &self,
        _error: &RpcError,
        _: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }
}
