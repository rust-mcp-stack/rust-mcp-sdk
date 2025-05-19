use async_trait::async_trait;

use rust_mcp_schema::{
    schema_utils::{CallToolError, NotificationFromClient, RequestFromClient, ResultFromServer},
    ClientRequest, ListToolsResult, RpcError,
};
use rust_mcp_sdk::{mcp_server::ServerHandlerCore, McpServer};

use crate::tools::GreetingTools;

pub struct MyServerHandler;

// To check out a list of all the methods in the trait that you can override, take a look at
// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_server_handler_core.rs
#[allow(unused)]
#[async_trait]
impl ServerHandlerCore for MyServerHandler {
    // Process incoming requests from the client
    async fn handle_request(
        &self,
        request: RequestFromClient,
        runtime: &dyn McpServer,
    ) -> std::result::Result<ResultFromServer, RpcError> {
        let method_name = &request.method().to_owned();
        match request {
            //Handle client requests according to their specific type.
            RequestFromClient::ClientRequest(client_request) => match client_request {
                // Handle the initialization request
                ClientRequest::InitializeRequest(_) => Ok(runtime.server_info().to_owned().into()),

                // Handle ListToolsRequest, return list of available tools
                ClientRequest::ListToolsRequest(_) => Ok(ListToolsResult {
                    meta: None,
                    next_cursor: None,
                    tools: GreetingTools::tools(),
                }
                .into()),

                // Handles incoming CallToolRequest and processes it using the appropriate tool.
                ClientRequest::CallToolRequest(request) => {
                    let tool_name = request.tool_name().to_string();

                    // Attempt to convert request parameters into GreetingTools enum
                    let tool_params = GreetingTools::try_from(request.params)
                        .map_err(|_| CallToolError::unknown_tool(tool_name.clone()))?;

                    // Match the tool variant and execute its corresponding logic
                    let result = match tool_params {
                        GreetingTools::SayHelloTool(say_hello_tool) => {
                            say_hello_tool.call_tool().map_err(|err| {
                                RpcError::internal_error().with_message(err.to_string())
                            })?
                        }
                        GreetingTools::SayGoodbyeTool(say_goodbye_tool) => {
                            say_goodbye_tool.call_tool().map_err(|err| {
                                RpcError::internal_error().with_message(err.to_string())
                            })?
                        }
                    };
                    Ok(result.into())
                }

                // Return Method not found for any other requests
                _ => Err(RpcError::method_not_found()
                    .with_message(format!("No handler is implemented for '{}'.", method_name,))),
            },
            // Handle custom requests
            RequestFromClient::CustomRequest(_) => Err(RpcError::method_not_found()
                .with_message("No handler is implemented for custom requests.".to_string())),
        }
    }

    // Process incoming client notifications
    async fn handle_notification(
        &self,
        notification: NotificationFromClient,
        _: &dyn McpServer,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    // Process incoming client errors
    async fn handle_error(
        &self,
        error: RpcError,
        _: &dyn McpServer,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }
}
