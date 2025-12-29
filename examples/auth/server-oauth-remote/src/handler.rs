use async_trait::async_trait;
use rust_mcp_sdk::auth::AuthInfo;
use rust_mcp_sdk::macros::{mcp_tool, JsonSchema};
use rust_mcp_sdk::schema::{
    schema_utils::CallToolError, CallToolResult, ListToolsResult, RpcError,
};
use rust_mcp_sdk::schema::{CallToolRequestParams, PaginatedRequestParams, TextContent};
use rust_mcp_sdk::{mcp_server::ServerHandler, McpServer};
use std::sync::Arc;
use std::vec;

//*******************************//
//  Show Authentication Info  //
//*******************************//
#[mcp_tool(
    name = "show_auth_info",
    description = "Shows current user authentication info in json format"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema, Default)]
pub struct ShowAuthInfo {}
impl ShowAuthInfo {
    pub fn call_tool(&self, auth_info: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let auth_info_json = serde_json::to_string_pretty(&auth_info).map_err(|err| {
            CallToolError::from_message(format!("Undable to display auth info as string :{err}"))
        })?;
        Ok(CallToolResult::text_content(vec![TextContent::from(
            auth_info_json,
        )]))
    }
}

// Custom Handler to handle MCP Messages
pub struct McpServerHandler;

// To check out a list of all the methods in the trait that you can override, take a look at
// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_server_handler.rs

#[async_trait]
#[allow(unused)]
impl ServerHandler for McpServerHandler {
    // Handle ListToolsRequest, return list of available tools as ListToolsResult
    async fn handle_list_tools_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: vec![ShowAuthInfo::tool()],
        })
    }

    /// Handles incoming CallToolRequest and processes it using the appropriate tool.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        if params.name.eq(&ShowAuthInfo::tool_name()) {
            let tool = ShowAuthInfo::default();
            tool.call_tool(runtime.auth_info_cloned().await)
        } else {
            Err(CallToolError::from_message(format!(
                "Tool \"{}\" does not exists or inactive!",
                params.name,
            )))
        }
    }
}
