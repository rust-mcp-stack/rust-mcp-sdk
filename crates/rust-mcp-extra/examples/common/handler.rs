use crate::common::tool::ShowAuthInfo;
use async_trait::async_trait;
use rust_mcp_sdk::{
    mcp_server::ServerHandler,
    schema::{
        schema_utils::CallToolError, CallToolRequestParams, ListToolsResult,
        ListToolsResultCacheScope, PaginatedRequestParams, RpcError, ServerResult,
    },
    McpServer, RequestContext,
};
use std::sync::Arc;

pub struct McpServerHandler;
#[async_trait]
impl ServerHandler for McpServerHandler {
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            cache_scope: ListToolsResultCacheScope::Public,
            meta: None,
            next_cursor: None,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            tools: vec![ShowAuthInfo::tool()],
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, CallToolError> {
        if params.name.eq(&ShowAuthInfo::tool_name()) {
            let tool = ShowAuthInfo::default();
            tool.call_tool(runtime.auth_info_cloned().await)
                .map(|r| r.into())
        } else {
            Err(CallToolError::from_message(format!(
                "Tool \"{}\" does not exists or inactive!",
                params.name,
            )))
        }
    }
}
