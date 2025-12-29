use crate::common::tool::ShowAuthInfo;
use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::ServerHandler,
    schema::{
        schema_utils::CallToolError, CallToolRequestParams, CallToolResult, ListToolsResult,
        PaginatedRequestParams, RpcError,
    },
    McpServer,
};
use std::sync::Arc;

pub struct McpServerHandler;
#[async_trait]
impl ServerHandler for McpServerHandler {
    // Handle ListToolsRequest, return list of available tools as ListToolsResult
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
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
