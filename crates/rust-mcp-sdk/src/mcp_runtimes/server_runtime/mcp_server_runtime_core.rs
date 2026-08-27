use super::ServerRuntime;
use crate::error::SdkResult;
use crate::mcp_handlers::mcp_server_handler_core::ServerHandlerCore;
use crate::mcp_runtimes::server_runtime::mcp_server_runtime::method_name_for_request;
use crate::mcp_runtimes::server_runtime::McpServerOptions;
use crate::mcp_traits::{McpServer, McpServerHandler, RequestContext};
use crate::schema::schema_utils::{ClientMessage, MessageFromServer, ServerMessage};
use crate::schema::{
    schema_utils::{ClientMessages, ServerMessages},
    RequestMetaObject, RpcError, ServerResult,
};
use async_trait::async_trait;
use rust_mcp_schema::schema_utils::{ClientJsonrpcNotification, ClientJsonrpcRequest};
use rust_mcp_schema::ClientRequest;
use rust_mcp_transport::TransportDispatcher;
use std::sync::Arc;

/// Creates a new MCP server runtime with the specified configuration.
///
/// This function initializes a server for (MCP) by accepting server details, transport ,
/// and a handler for server-side logic.
/// The resulting `ServerRuntime` manages the server's operation and communication with MCP clients.
///
/// # Arguments
/// * `server_details` - Server name , version and capabilities.
/// * `transport` - An implementation of the `Transport` trait facilitating communication with the MCP clients.
/// * `handler` - An implementation of the `ServerHandlerCore` trait that defines the server's core behavior and response logic.
///
/// # Returns
/// A `ServerRuntime` instance representing the initialized server, ready for asynchronous operation.
///
/// # Examples
/// You can find a detailed example of how to use this function in the repository:
///
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-stdio-core)
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

pub(crate) struct RuntimeCoreInternalHandler<H> {
    handler: H,
}

impl RuntimeCoreInternalHandler<Box<dyn ServerHandlerCore>> {
    pub fn new(handler: Box<dyn ServerHandlerCore>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl McpServerHandler for RuntimeCoreInternalHandler<Box<dyn ServerHandlerCore>> {
    async fn handle_request(
        &self,
        client_jsonrpc_request: ClientJsonrpcRequest,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        let context = match &client_jsonrpc_request {
            ClientJsonrpcRequest::Standard(standard) => {
                RequestContext::from_request_meta(request_meta(standard))?
            }
            ClientJsonrpcRequest::Custom(_) => RequestContext::empty(),
        };
        if let ClientJsonrpcRequest::Standard(ref standard) = &client_jsonrpc_request {
            let method = method_name_for_request(standard);
            context.ensure_capabilities(
                method,
                &self.handler.required_capabilities_for_method(method),
            )?;
        }
        self.handler
            .handle_request(client_jsonrpc_request.into(), &context, runtime)
            .await
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
        // handle notification
        self.handler
            .handle_notification(client_jsonrpc_notification.into(), runtime)
            .await?;
        Ok(())
    }
}

fn request_meta(request: &ClientRequest) -> &RequestMetaObject {
    use rust_mcp_schema::ClientRequest::*;
    match request {
        DiscoverRequest(r) => &r.params.meta,
        ListResourcesRequest(r) => &r.params.meta,
        ListResourceTemplatesRequest(r) => &r.params.meta,
        ReadResourceRequest(r) => &r.params.meta,
        SubscriptionsListenRequest(r) => &r.params.meta,
        ListPromptsRequest(r) => &r.params.meta,
        GetPromptRequest(r) => &r.params.meta,
        ListToolsRequest(r) => &r.params.meta,
        CallToolRequest(r) => &r.params.meta,
        CompleteRequest(r) => &r.params.meta,
    }
}
