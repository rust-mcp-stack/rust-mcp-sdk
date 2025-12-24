use super::ClientRuntime;
use super::McpClientOptions;
use crate::schema::{
    schema_utils::{
        ClientMessage, ClientMessages, MessageFromClient, NotificationFromServer, ResultFromClient,
        ServerMessage, ServerMessages,
    },
    InitializeRequestParams, RpcError,
};
#[cfg(feature = "streamable-http")]
use crate::task_store::ClientTaskStore;
use crate::{
    error::SdkResult,
    mcp_handlers::mcp_client_handler_core::ClientHandlerCore,
    mcp_traits::{McpClient, McpClientHandler},
};
use async_trait::async_trait;
use rust_mcp_schema::schema_utils::ServerJsonrpcRequest;
#[cfg(feature = "streamable-http")]
use rust_mcp_transport::StreamableTransportOptions;
use rust_mcp_transport::TransportDispatcher;
use std::sync::Arc;

/// Creates a new MCP client runtime with the specified options.
///
/// This function initializes an MCP client runtime by taking a bundled `McpClientOptions<T>` struct
/// that contains all necessary configuration components.
///
/// The resulting `ClientRuntime` is wrapped in an `Arc` to enable safe sharing and concurrent use
/// across asynchronous tasks.
///
/// # Arguments
///
/// * `options` - A `McpClientOptions<T>` struct containing:
///   - `client_details`: Details about the client, including name, version, and capabilities.
///   - `transport`: An implementation of the `TransportDispatcher` trait for communication with the MCP server.
///   - `handler`: The client's core handler (typically a boxed `dyn ClientHandlerCore` or similar)
///     that defines the client's behavior and response logic.
///   - `task_store`: Optional task storage for managing asynchronous operations (if applicable).
///
/// # Returns
///
/// An `Arc<ClientRuntime>` representing the initialized client runtime, ready for shared ownership
/// and asynchronous operation.
///
/// # Examples
///
/// You can find a detailed example of how to use this function in the repository:
///
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client-stdio-core)
pub fn create_client<T>(options: McpClientOptions<T>) -> Arc<ClientRuntime>
where
    T: TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    >,
{
    Arc::new(ClientRuntime::new(
        options.client_details,
        Arc::new(options.transport),
        options.handler,
        options.task_store,
    ))
}

#[cfg(feature = "streamable-http")]
pub fn with_transport_options(
    client_details: InitializeRequestParams,
    transport_options: StreamableTransportOptions,
    handler: impl ClientHandlerCore,
    task_store: Option<Arc<ClientTaskStore>>,
) -> Arc<ClientRuntime> {
    Arc::new(ClientRuntime::new_instance(
        client_details,
        transport_options,
        Box::new(ClientCoreInternalHandler::new(Box::new(handler))),
        task_store,
    ))
}

pub(crate) struct ClientCoreInternalHandler<H> {
    handler: H,
}

impl ClientCoreInternalHandler<Box<dyn ClientHandlerCore>> {
    pub fn new(handler: Box<dyn ClientHandlerCore>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl McpClientHandler for ClientCoreInternalHandler<Box<dyn ClientHandlerCore>> {
    async fn handle_request(
        &self,
        server_jsonrpc_request: ServerJsonrpcRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        // handle request and get the result
        self.handler
            .handle_request(server_jsonrpc_request, runtime)
            .await
    }

    async fn handle_error(
        &self,
        jsonrpc_error: &RpcError,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        self.handler.handle_error(jsonrpc_error, runtime).await?;
        Ok(())
    }
    async fn handle_notification(
        &self,
        server_jsonrpc_notification: NotificationFromServer,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        // handle notification
        self.handler
            .handle_notification(server_jsonrpc_notification, runtime)
            .await?;
        Ok(())
    }

    async fn handle_process_error(
        &self,
        error_message: String,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        self.handler
            .handle_process_error(error_message, runtime)
            .await
            .map_err(|err| err.into())
    }
}
