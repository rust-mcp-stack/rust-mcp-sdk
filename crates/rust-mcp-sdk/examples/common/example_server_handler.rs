use super::tools::GreetingTools;
use async_trait::async_trait;
use rust_mcp_schema::{
    ListResourceTemplatesRequest, ListResourceTemplatesResult, ListResourcesRequest,
    ListResourcesResult, ReadResourceRequest, ReadResourceRequestParams, ReadResourceResult,
    Resource, ResourceTemplate,
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
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resources_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        let resource: Resource = Resource {
            name: "ResourceName".to_string(),
            description: None,
            meta: None,
            title: None,
            icons: vec![],
            annotations: None,
            mime_type: None,
            size: None,
            uri: "".to_string(),
        };

        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ListResourcesRequest::method_value(),
        )))
    }

    /// Handles requests to list resource templates.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resource_templates_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, RpcError> {
        let template: ResourceTemplate = ResourceTemplate {
            name: "TemplateName".to_string(),
            description: None,
            meta: None,
            title: None,
            icons: vec![],
            mime_type: None,
            uri_template: "Template".to_string(),
            annotations: None,
        };
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ListResourceTemplatesRequest::method_value(),
        )))
    }

    /// Handles requests to read a specific resource.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ReadResourceRequest::method_value(),
        )))
    }
}
