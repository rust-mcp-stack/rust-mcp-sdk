use super::tools::GreetingTools;
use crate::common::resources::{BlobTextResource, PlainTextResource, PokemonImageResource};
use async_trait::async_trait;
use rust_mcp_schema::{
    CompleteRequestParams, CompleteResult, ListResourceTemplatesResult, ListResourcesResult,
    ReadResourceRequestParams, ReadResourceResult,
};
use rust_mcp_sdk::{
    mcp_server::ServerHandler,
    schema::{
        schema_utils::CallToolError, CallToolRequestParams, CallToolResult, ListToolsResult,
        PaginatedRequestParams, RpcError,
    },
    McpServer,
};
use std::sync::Arc;

// Custom Handler to handle MCP Messages
pub struct ExampleServerHandler;

// To check out a list of all the methods in the trait that you can override, take a look at
// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_server_handler.rs

#[async_trait]
#[allow(unused)]
impl ServerHandler for ExampleServerHandler {
    // Handle ListToolsRequest, return list of available tools as ListToolsResult
    async fn handle_list_tools_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: GreetingTools::tools(),
        })
    }

    /// Handles incoming CallToolRequest and processes it using the appropriate tool.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        // Attempt to convert request parameters into GreetingTools enum
        let tool_params: GreetingTools =
            GreetingTools::try_from(params).map_err(CallToolError::new)?;

        // Match the tool variant and execute its corresponding logic
        match tool_params {
            GreetingTools::SayHelloTool(say_hello_tool) => say_hello_tool.call_tool(),
            GreetingTools::SayGoodbyeTool(say_goodbye_tool) => say_goodbye_tool.call_tool(),
        }
    }

    /// Handles requests to list available resources.
    ///
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resources_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Ok(ListResourcesResult {
            meta: None,
            next_cursor: None,
            resources: vec![PlainTextResource::resource(), BlobTextResource::resource()],
        })
    }

    /// Handles requests to list resource templates.
    ///
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resource_templates_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, RpcError> {
        Ok(ListResourceTemplatesResult {
            meta: None,
            next_cursor: None,
            resource_templates: vec![PokemonImageResource::resource_template()],
        })
    }

    /// Handles requests to read a specific resource.
    ///
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        if PlainTextResource::resource_uri().starts_with(&params.uri) {
            return PlainTextResource::get_resource().await;
        }
        if BlobTextResource::resource_uri().starts_with(&params.uri) {
            return BlobTextResource::get_resource().await;
        }

        if PokemonImageResource::matches_url(&params.uri) {
            return PokemonImageResource::get_resource(&params.uri).await;
        }

        Err(RpcError::invalid_request()
            .with_message(format!("No resource was found for '{}'.", params.uri)))
    }

    async fn handle_complete_request(
        &self,
        params: CompleteRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CompleteResult, RpcError> {
        if params.argument.name.eq("pokemon-id") {
            Ok(CompleteResult {
                completion: PokemonImageResource::completion(&params.argument.value),
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
