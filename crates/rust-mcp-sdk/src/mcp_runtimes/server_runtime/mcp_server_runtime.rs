use super::ServerRuntime;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::auth::AuthInfo;
use crate::mcp_traits::RequestContext;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::mcp_traits::ServerDetails;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::McpObserver;
use crate::{
    error::SdkResult,
    mcp_handlers::mcp_server_handler::ServerHandler,
    mcp_traits::{McpServer, McpServerHandler},
};
use crate::{
    mcp_runtimes::server_runtime::McpServerOptions,
    schema::{
        schema_utils::{
            ClientMessage, ClientMessages, MessageFromServer, ServerMessage, ServerMessages,
        },
        NotificationMetaObject, RequestMetaObject, RpcError, ServerResult,
        SubscriptionsAcknowledgedNotificationParams,
    },
};
use async_trait::async_trait;
use rust_mcp_schema::schema_utils::{ClientJsonrpcNotification, ClientJsonrpcRequest};
use rust_mcp_schema::ClientRequest;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use rust_mcp_transport::SessionId;
use rust_mcp_transport::TransportDispatcher;
use std::sync::Arc;

/// Returns a reference to the `RequestMetaObject` for any standard 2026-07-28
/// `ClientRequest`. All request-param structs carry a required `meta` field.
fn request_meta(request: &ClientRequest) -> &RequestMetaObject {
    match request {
        ClientRequest::DiscoverRequest(r) => &r.params.meta,
        ClientRequest::ListResourcesRequest(r) => &r.params.meta,
        ClientRequest::ListResourceTemplatesRequest(r) => &r.params.meta,
        ClientRequest::ReadResourceRequest(r) => &r.params.meta,
        ClientRequest::SubscriptionsListenRequest(r) => &r.params.meta,
        ClientRequest::ListPromptsRequest(r) => &r.params.meta,
        ClientRequest::GetPromptRequest(r) => &r.params.meta,
        ClientRequest::ListToolsRequest(r) => &r.params.meta,
        ClientRequest::CallToolRequest(r) => &r.params.meta,
        ClientRequest::CompleteRequest(r) => &r.params.meta,
    }
}
///
/// This function initializes a server for (MCP) by accepting server details, transport ,
/// and a handler for server-side logic.
/// The resulting `ServerRuntime` manages the server's operation and communication with MCP clients.
///
/// # Arguments
/// * `server_details` - Server name , version and capabilities.
/// * `transport` - An implementation of the `Transport` trait facilitating communication with the MCP clients.
/// * `handler` - An implementation of the `ServerHandler` trait that defines the server's core behavior and response logic.
///
/// # Returns
/// A `ServerRuntime` instance representing the initialized server, ready for asynchronous operation.
///
/// # Examples
/// You can find a detailed example of how to use this function in the repository:
///
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-stdio)
pub fn create_server<T>(options: McpServerOptions<T>) -> Arc<ServerRuntime>
where
    T: TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    >,
{
    ServerRuntime::new(options)
}

#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub(crate) fn create_server_instance(
    server_details: Arc<ServerDetails>,
    handler: Arc<dyn McpServerHandler>,
    session_id: SessionId,
    auth_info: Option<AuthInfo>,
    message_observer: Option<Arc<dyn McpObserver<ClientMessage, ServerMessage>>>,
) -> Arc<ServerRuntime> {
    ServerRuntime::new_instance(
        server_details,
        handler,
        session_id,
        auth_info,
        message_observer,
    )
}

/// Maps a 2026-07-28 `ClientRequest` to its JSON-RPC method name.
pub(crate) fn method_name_for_request(request: &ClientRequest) -> &'static str {
    match request {
        ClientRequest::DiscoverRequest(_) => "server/discover",
        ClientRequest::ListResourcesRequest(_) => "resources/list",
        ClientRequest::ListResourceTemplatesRequest(_) => "resources/templates/list",
        ClientRequest::ReadResourceRequest(_) => "resources/read",
        ClientRequest::SubscriptionsListenRequest(_) => "subscriptions/listen",
        ClientRequest::ListPromptsRequest(_) => "prompts/list",
        ClientRequest::GetPromptRequest(_) => "prompts/get",
        ClientRequest::ListToolsRequest(_) => "tools/list",
        ClientRequest::CallToolRequest(_) => "tools/call",
        ClientRequest::CompleteRequest(_) => "completion/complete",
    }
}

/// Stamps the server's identity onto a `ServerResult`'s `_meta`
/// (`io.modelcontextprotocol/serverInfo`) when the result already carries
/// a `meta` field.  This is a SHOULD in the 2026-07-28 spec.
fn stamp_server_info(result: &mut ServerResult, server_info: &rust_mcp_schema::Implementation) {
    use crate::schema::ResultMetaObject;
    let meta: &mut ResultMetaObject = match result {
        ServerResult::DiscoverResult(r) => r.meta.get_or_insert_with(ResultMetaObject::default),
        ServerResult::ListResourcesResult(r) => {
            r.meta.get_or_insert_with(ResultMetaObject::default)
        }
        ServerResult::ListResourceTemplatesResult(r) => {
            r.meta.get_or_insert_with(ResultMetaObject::default)
        }
        ServerResult::ReadResourceResult(r) => r.meta.get_or_insert_with(ResultMetaObject::default),
        ServerResult::ListPromptsResult(r) => r.meta.get_or_insert_with(ResultMetaObject::default),
        ServerResult::GetPromptResult(r) => r.meta.get_or_insert_with(ResultMetaObject::default),
        ServerResult::ListToolsResult(r) => r.meta.get_or_insert_with(ResultMetaObject::default),
        ServerResult::CallToolResult(r) => r.meta.get_or_insert_with(ResultMetaObject::default),
        ServerResult::CompleteResult(r) => r.meta.get_or_insert_with(ResultMetaObject::default),
        ServerResult::SubscriptionsListenResult(r) => {
            r.meta.io_modelcontextprotocol_server_info = Some(server_info.clone());
            return;
        }
        _ => return,
    };
    meta.io_modelcontextprotocol_server_info = Some(server_info.clone());
}

pub(crate) struct ServerRuntimeInternalHandler<H> {
    handler: H,
}
impl ServerRuntimeInternalHandler<Box<dyn ServerHandler>> {
    pub fn new(handler: Box<dyn ServerHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl McpServerHandler for ServerRuntimeInternalHandler<Box<dyn ServerHandler>> {
    async fn handle_request(
        &self,
        client_jsonrpc_request: ClientJsonrpcRequest,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        let server_info = runtime.server_details().server_info.clone();
        let mut result = match client_jsonrpc_request {
            ClientJsonrpcRequest::Standard(standard_request) => {
                let meta = request_meta(&standard_request);
                let context = RequestContext::from_request_meta(meta)?;
                runtime.set_active_log_level(context.log_level.clone());
                let method = method_name_for_request(&standard_request);
                context.ensure_capabilities(
                    method,
                    &self.handler.required_capabilities_for_method(method),
                )?;
                let output: std::result::Result<ServerResult, RpcError> = match standard_request {
                    ClientRequest::ListResourcesRequest(list_resources_request) => self
                        .handler
                        .handle_list_resources_request(
                            Some(list_resources_request.params),
                            &context,
                            runtime,
                        )
                        .await
                        .map(Into::into),
                    ClientRequest::ListResourceTemplatesRequest(
                        list_resource_templates_request,
                    ) => self
                        .handler
                        .handle_list_resource_templates_request(
                            Some(list_resource_templates_request.params),
                            &context,
                            runtime,
                        )
                        .await
                        .map(Into::into),
                    ClientRequest::ReadResourceRequest(read_resource_request) => {
                        self.handler
                            .handle_read_resource_request(
                                read_resource_request.params,
                                &context,
                                runtime,
                            )
                            .await
                    }
                    ClientRequest::ListPromptsRequest(list_prompts_request) => self
                        .handler
                        .handle_list_prompts_request(
                            Some(list_prompts_request.params),
                            &context,
                            runtime,
                        )
                        .await
                        .map(Into::into),
                    ClientRequest::GetPromptRequest(prompt_request) => {
                        self.handler
                            .handle_get_prompt_request(prompt_request.params, &context, runtime)
                            .await
                    }
                    ClientRequest::ListToolsRequest(list_tools_request) => self
                        .handler
                        .handle_list_tools_request(
                            Some(list_tools_request.params),
                            &context,
                            runtime,
                        )
                        .await
                        .map(Into::into),
                    ClientRequest::CallToolRequest(call_tool_request) => {
                        // Per-tool capability gate (SEP-2575): a server MUST NOT
                        // rely on a capability the client has not declared.
                        context.ensure_capabilities(
                            "tools/call",
                            &self.handler.required_capabilities_for_tool_call(
                                &call_tool_request.params.name,
                            ),
                        )?;
                        // MRTR-aware dispatch (SEP-2322): the handler may return
                        // an InputRequiredResult or a protocol-level RpcError.
                        self.handler
                            .handle_call_tool_request_mrtr(
                                call_tool_request.params,
                                &context,
                                runtime,
                            )
                            .await
                    }
                    ClientRequest::CompleteRequest(complete_request) => self
                        .handler
                        .handle_complete_request(complete_request.params, &context, runtime)
                        .await
                        .map(Into::into),
                    ClientRequest::DiscoverRequest(discover_request) => self
                        .handler
                        .handle_discover_request(discover_request.params, &context, runtime)
                        .await
                        .map(Into::into),
                    ClientRequest::SubscriptionsListenRequest(req) => {
                        let resource_count = req.params.notifications.resource_subscriptions.len();
                        if !runtime.is_within_subscription_limit(resource_count) {
                            return Err(RpcError::new(
                                rust_mcp_schema::schema_utils::RpcErrorCodes::MISSING_REQUIRED_CLIENT_CAPABILITY,
                                format!("Subscription limit exceeded: {resource_count} resources requested"),
                                None,
                            ));
                        }
                        let result = self
                            .handler
                            .handle_subscriptions_listen_request(
                                req.id.clone(),
                                req.params,
                                &context,
                                runtime.clone(),
                            )
                            .await;
                        // Send acknowledgment with the listen request's id as
                        // `subscriptionId`, per TS SDK semantics.
                        if result.is_ok() {
                            runtime.stream_started();
                            // Register this request's transport as the session's
                            // notification channel so subscription-scoped
                            // notifications find the right listen stream.
                            runtime.register_notification_transport();
                            if let Some(filter) = runtime.subscription_filter() {
                                let ack = SubscriptionsAcknowledgedNotificationParams {
                                    notifications: filter,
                                    meta: Some(NotificationMetaObject {
                                        io_modelcontextprotocol_subscription_id: Some(req.id),
                                        ..Default::default()
                                    }),
                                };
                                let _ = runtime.notify_subscriptions_acknowledged(ack).await;
                            }
                        }
                        result.map(Into::into)
                    }
                };
                if matches!(output, Ok(ServerResult::InputRequiredResult(_)))
                    && !matches!(method, "tools/call" | "prompts/get" | "resources/read")
                {
                    return Err(RpcError::internal_error()
                        .with_message("InputRequiredResult not allowed for this method"));
                }
                output
            }
            ClientJsonrpcRequest::Custom(custom_request) => {
                let context = RequestContext::empty();
                self.handler
                    .handle_custom_request(custom_request.into(), &context, runtime)
                    .await
                    .map(Into::into)
            }
        }?;
        stamp_server_info(&mut result, &server_info);
        Ok(result)
    }

    async fn handle_error(
        &self,
        jsonrpc_error: &RpcError,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()> {
        self.handler.handle_error(jsonrpc_error, runtime).await?;
        Ok(())
    }

    async fn handle_notification(
        &self,
        client_jsonrpc_notification: ClientJsonrpcNotification,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()> {
        match client_jsonrpc_notification {
            ClientJsonrpcNotification::CancelledNotification(cancelled_notification) => {
                self.handler
                    .handle_cancelled_notification(cancelled_notification.params, runtime)
                    .await?;
            }
            // NOTE (2026-07-28): the client→server notification vocabulary was reduced to
            // `notifications/cancelled`; anything else arrives via this Custom channel.
            ClientJsonrpcNotification::CustomNotification(value) => {
                self.handler
                    .handle_custom_notification(value.into())
                    .await?;
            }
        }
        Ok(())
    }

    fn tool_header_annotations(
        &self,
        tool_name: &str,
    ) -> Vec<crate::tool_param_headers::ToolParamHeader> {
        self.handler.tool_header_annotations(tool_name)
    }
}
