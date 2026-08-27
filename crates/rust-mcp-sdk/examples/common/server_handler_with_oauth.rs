use async_trait::async_trait;
use rust_mcp_sdk::auth::AuthInfo;
use rust_mcp_sdk::schema::{
    schema_utils::CallToolError, CallToolRequestParams, CallToolResult, ListToolsResult,
    ListToolsResultCacheScope, PaginatedRequestParams, RpcError, ServerResult, TextContent,
};
use rust_mcp_sdk::{
    macros::{mcp_tool, JsonSchema},
    mcp_server::ServerHandler,
    McpServer, RequestContext,
};
use std::sync::Arc;

#[mcp_tool(
    name = "show_auth_info",
    description = "Shows current user authentication info in json format"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema, Default)]
pub struct ShowAuthInfo {}
impl ShowAuthInfo {
    pub fn call_tool(&self, auth_info: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let auth_info_json = serde_json::to_string_pretty(&auth_info).map_err(|err| {
            CallToolError::from_message(format!("Unable to display auth info as string: {err}"))
        })?;
        Ok(CallToolResult::text_content(vec![TextContent::from(
            auth_info_json,
        )]))
    }
}

pub struct ServerHandlerAuth;

#[async_trait]
#[allow(unused)]
impl ServerHandler for ServerHandlerAuth {
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
                "Tool \"{}\" does not exist or inactive!",
                params.name,
            )))
        }
    }
}
