use crate::mcp_traits::McpServer;
use crate::mcp_traits::RequestContext;
use crate::mcp_traits::RequiredClientCapability;
use crate::{
    mcp_server::server_runtime::ServerRuntimeInternalHandler,
    mcp_traits::{McpServerHandler, ToMcpServerHandler},
    schema::{
        schema_utils::{CallToolError, CustomNotification, CustomRequest},
        *,
    },
};
use async_trait::async_trait;
use std::sync::Arc;

/// The `ServerHandler` trait defines how a server handles Model Context Protocol (MCP) operations.
/// It provides default implementations for request , notification and error handlers, and must be extended or
/// overridden by developers to customize server behavior.
#[allow(unused)]
#[async_trait]
pub trait ServerHandler: Send + Sync + 'static {
    /// Returns the set of client capabilities this handler requires for
    /// the given method.  Before dispatching a request the runtime calls
    /// this method and rejects the request with
    /// [`MISSING_REQUIRED_CLIENT_CAPABILITY`] (-32021) when the client
    /// did not declare every required capability.
    fn required_capabilities_for_method(&self, _method: &str) -> Vec<RequiredClientCapability> {
        Vec::new()
    }

    /// Returns the set of client capabilities required to call the given
    /// tool. Before dispatching a `tools/call`, the runtime rejects the
    /// request with [`MISSING_REQUIRED_CLIENT_CAPABILITY`] (-32021) when the
    /// client did not declare every required capability — so a server never
    /// relies on a capability the client has not advertised.
    fn required_capabilities_for_tool_call(
        &self,
        _tool_name: &str,
    ) -> Vec<RequiredClientCapability> {
        Vec::new()
    }

    /// SEP-2243 custom-header validation: returns the `x-mcp-header`
    /// annotations of the given tool, so the HTTP layer can validate
    /// `Mcp-Param-*` headers against the request body.
    ///
    /// Defaults to empty. Override with [`crate::tool_param_headers::annotations_for_tool`]
    /// to parse annotations from your tool definitions.
    fn tool_header_annotations(
        &self,
        _tool_name: &str,
    ) -> Vec<crate::tool_param_headers::ToolParamHeader> {
        Vec::new()
    }

    /// Handles requests to list available resources.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resources_request(
        &self,
        params: Option<PaginatedRequestParams>,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ListResourcesRequest::method_value(),
        )))
    }

    /// Handles requests to list resource templates.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resource_templates_request(
        &self,
        params: Option<PaginatedRequestParams>,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ListResourceTemplatesRequest::method_value(),
        )))
    }

    /// Handles requests to read a specific resource.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ReadResourceRequest::method_value(),
        )))
    }

    /// Handles requests to list available prompts.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_prompts_request(
        &self,
        params: Option<PaginatedRequestParams>,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ListPromptsRequest::method_value(),
        )))
    }

    /// Handles requests to get a specific prompt.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_get_prompt_request(
        &self,
        params: GetPromptRequestParams,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            GetPromptRequest::method_value(),
        )))
    }

    /// Handles requests to list available tools.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_tools_request(
        &self,
        params: Option<PaginatedRequestParams>,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ListToolsRequest::method_value(),
        )))
    }

    /// Handles requests to call a specific tool.
    ///
    /// Default implementation returns an unknown tool error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, CallToolError> {
        let result: CallToolResult =
            CallToolError::unknown_tool(format!("Unknown tool: {}", params.name)).into();
        Ok(result.into())
    }

    /// MRTR-aware variant of [`handle_call_tool_request`](Self::handle_call_tool_request)
    /// (SEP-2322).
    ///
    /// Unlike the legacy method — whose `CallToolError` is always converted
    /// into a `CallToolResult` with `isError: true` — this method returns a
    /// protocol-level [`RpcError`], which becomes the JSON-RPC error response
    /// of the request. Use it when a tool call must:
    ///
    /// - return an [`InputRequiredResult`] to drive a mid-request turn-around
    ///   (resolved `inputResponses` arrive on the retry via `params`), or
    /// - reject the call with a JSON-RPC error (e.g. `-32602` when an
    ///   integrity-protected `requestState` fails verification).
    ///
    /// The default implementation preserves the legacy behavior exactly:
    /// it delegates to [`handle_call_tool_request`](Self::handle_call_tool_request)
    /// and converts a `CallToolError` into an `isError` result.
    async fn handle_call_tool_request_mrtr(
        &self,
        params: CallToolRequestParams,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        match self
            .handle_call_tool_request(params, context, runtime)
            .await
        {
            Ok(result) => Ok(result),
            Err(tool_error) => {
                let result: CallToolResult = tool_error.into();
                Ok(result.into())
            }
        }
    }

    /// Handles completion requests from clients.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_complete_request(
        &self,
        params: CompleteRequestParams,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CompleteResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            CompleteRequest::method_value(),
        )))
    }

    /// Handles custom requests not defined in the standard protocol.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_custom_request(
        &self,
        request: CustomRequest,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<GenericResult, RpcError> {
        Err(RpcError::method_not_found()
            .with_message("No handler is implemented for custom requests.".to_string()))
    }

    /// Handles `server/discover` — advertises the server's capabilities,
    /// supported protocol versions, and instructions.
    ///
    /// The default implementation builds the response from the runtime's
    /// `server_details()`. Servers that need dynamic version negotiation
    /// or cache directives can override this.
    async fn handle_discover_request(
        &self,
        _params: RequestParams,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<DiscoverResult, RpcError> {
        let details = runtime.server_details();
        Ok(DiscoverResult {
            capabilities: details.capabilities.clone(),
            instructions: details.instructions.clone(),
            meta: details.meta.clone(),
            result_type: "complete".to_string(),
            cache_scope: DiscoverResultCacheScope::Private,
            ttl_ms: 0,
            supported_versions: crate::utils::supported_protocol_versions(),
        })
    }
    /// Handles `subscriptions/listen` — a client requests notification
    /// subscriptions on a persistent stream.
    ///
    /// The default implementation stores the requested subscription filter on the
    /// runtime, sends a `notifications/subscriptions/acknowledged` (via the
    /// dispatcher), and returns an immediate `SubscriptionsListenResult` signaling
    /// teardown. Override this to implement long-lived subscription streams.
    async fn handle_subscriptions_listen_request(
        &self,
        request_id: RequestId,
        params: SubscriptionsListenRequestParams,
        _context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<SubscriptionsListenResult, RpcError> {
        runtime.store_subscription(params.notifications);
        let details = runtime.server_details();
        Ok(SubscriptionsListenResult {
            meta: SubscriptionsListenResultMetaObject {
                io_modelcontextprotocol_server_info: Some(details.server_info.clone()),
                io_modelcontextprotocol_subscription_id: request_id,
                extra: None,
            },
            result_type: "complete".to_string(),
        })
    }
    // Notification Handlers

    /// Handles cancelled operation notifications.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_cancelled_notification(
        &self,
        params: CancelledNotificationParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles custom notifications not defined in the standard protocol.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_custom_notification(
        &self,
        notification: CustomNotification,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    // Error Handler

    /// Handles server errors that occur during operation.
    ///
    /// # Arguments
    /// * `error` - The error that occurred
    /// * `runtime` - Reference to the MCP server runtime
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_error(
        &self,
        error: &RpcError,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }
}

impl<T: ServerHandler + 'static> ToMcpServerHandler for T {
    fn to_mcp_server_handler(self) -> Arc<dyn McpServerHandler + 'static> {
        Arc::new(ServerRuntimeInternalHandler::new(Box::new(self)))
    }
}
