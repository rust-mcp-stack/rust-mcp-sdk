#[cfg(feature = "hyper-server")]
pub mod test_server_common {
    use crate::common::sample_tools::SayHelloTool;
    use async_trait::async_trait;
    use rust_mcp_schema::schema_utils::CallToolError;
    use rust_mcp_schema::{
        CallToolRequest, CallToolResult, ListToolsRequest, ListToolsResult, ProtocolVersion,
        RpcError,
    };
    use rust_mcp_sdk::event_store::EventStore;
    use rust_mcp_sdk::id_generator::IdGenerator;
    use rust_mcp_sdk::mcp_server::hyper_runtime::HyperRuntime;
    use rust_mcp_sdk::schema::{
        ClientCapabilities, Implementation, InitializeRequest, InitializeRequestParams,
        InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
    };
    use rust_mcp_sdk::{
        mcp_server::{
            hyper_server, HyperServer, HyperServerOptions, ServerHandler, ToMcpServerHandler,
        },
        McpServer, SessionId,
    };
    use std::sync::{Arc, RwLock};
    use std::time::Duration;
    use tokio::time::timeout;
    use tokio_stream::StreamExt;

    pub const INITIALIZE_REQUEST: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{"sampling":{},"roots":{"listChanged":true}},"clientInfo":{"name":"reqwest-test","version":"0.1.0"}}}"#;
    pub const PING_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    pub const INITIALIZE_RESPONSE: &str = r#"{"result":{"protocolVersion":"2025-06-18","capabilities":{"prompts":{},"resources":{"subscribe":true},"tools":{},"logging":{}},"serverInfo":{"name":"example-servers/everything","version":"1.0.0"}},"jsonrpc":"2.0","id":0}"#;

    pub struct LaunchedServer {
        pub hyper_runtime: HyperRuntime,
        pub streamable_url: String,
        pub sse_url: String,
        pub sse_message_url: String,
        pub event_store: Option<Arc<dyn EventStore>>,
    }

    pub fn initialize_request() -> InitializeRequest {
        InitializeRequest::new(InitializeRequestParams {
            capabilities: ClientCapabilities {
                ..Default::default()
            },
            client_info: Implementation {
                name: "test-server".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            protocol_version: ProtocolVersion::V2025_06_18.to_string(),
        })
    }

    pub fn test_server_details() -> InitializeResult {
        InitializeResult {
            // server name and version
            server_info: Implementation {
                name: "Test MCP Server".to_string(),
                version: "0.1.0".to_string(),
                #[cfg(feature = "2025_06_18")]
                title: None,
            },
            capabilities: ServerCapabilities {
                // indicates that server support mcp tools
                tools: Some(ServerCapabilitiesTools { list_changed: None }),
                ..Default::default() // Using default values for other fields
            },
            meta: None,
            instructions: Some("server instructions...".to_string()),
            protocol_version: ProtocolVersion::V2025_06_18.to_string(),
        }
    }

    pub struct TestServerHandler;

    #[async_trait]
    impl ServerHandler for TestServerHandler {
        async fn handle_list_tools_request(
            &self,
            request: ListToolsRequest,
            runtime: Arc<dyn McpServer>,
        ) -> std::result::Result<ListToolsResult, RpcError> {
            runtime.assert_server_request_capabilities(request.method())?;

            Ok(ListToolsResult {
                meta: None,
                next_cursor: None,
                tools: vec![SayHelloTool::tool()],
            })
        }

        async fn handle_call_tool_request(
            &self,
            request: CallToolRequest,
            runtime: Arc<dyn McpServer>,
        ) -> std::result::Result<CallToolResult, CallToolError> {
            runtime
                .assert_server_request_capabilities(request.method())
                .map_err(CallToolError::new)?;
            if request.params.name != "say_hello" {
                Ok(
                    CallToolError::unknown_tool(format!("Unknown tool: {}", request.params.name))
                        .into(),
                )
            } else {
                let tool = SayHelloTool {
                    name: request.params.arguments.unwrap()["name"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                };

                Ok(tool.call_tool().unwrap())
            }
        }
    }

    pub fn create_test_server(options: HyperServerOptions) -> HyperServer {
        hyper_server::create_server(
            test_server_details(),
            TestServerHandler {}.to_mcp_server_handler(),
            options,
        )
    }

    pub async fn create_start_server(options: HyperServerOptions) -> LaunchedServer {
        let streamable_url = options.streamable_http_url();
        let sse_url = options.sse_url();
        let sse_message_url = options.sse_message_url();

        let event_store_clone = options.event_store.clone();
        let server = hyper_server::create_server(
            test_server_details(),
            TestServerHandler {}.to_mcp_server_handler(),
            options,
        );

        let hyper_runtime = HyperRuntime::create(server).await.unwrap();

        tokio::time::sleep(Duration::from_millis(75)).await;

        LaunchedServer {
            hyper_runtime,
            streamable_url,
            sse_url,
            sse_message_url,
            event_store: event_store_clone,
        }
    }

    // Tests the session ID generator, ensuring it returns a sequence of predefined session IDs.
    pub struct TestIdGenerator {
        constant_ids: Vec<SessionId>,
        generated: RwLock<usize>,
    }

    impl TestIdGenerator {
        pub fn new(constant_ids: Vec<SessionId>) -> Self {
            TestIdGenerator {
                constant_ids,
                generated: RwLock::new(0),
            }
        }
    }

    impl<T> IdGenerator<T> for TestIdGenerator
    where
        T: From<String>,
    {
        fn generate(&self) -> T {
            let mut lock = self.generated.write().unwrap();
            *lock += 1;
            if *lock > self.constant_ids.len() {
                *lock = 1;
            }
            T::from(self.constant_ids[*lock - 1].to_owned())
        }
    }

    pub async fn collect_sse_lines(
        response: reqwest::Response,
        line_count: usize,
        read_timeout: Duration,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut collected_lines = Vec::new();
        let mut stream = response.bytes_stream();

        let result = timeout(read_timeout, async {
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                let chunk_str = String::from_utf8_lossy(&chunk);

                // Split the chunk into lines
                let lines: Vec<&str> = chunk_str.lines().collect();

                // Add each line to the collected_lines vector
                for line in lines {
                    collected_lines.push(line.to_string());

                    // Check if we have collected 5 lines
                    if collected_lines.len() >= line_count {
                        return Ok(collected_lines);
                    }
                }
            }
            // If the stream ends before collecting 5 lines, return what we have
            Ok(collected_lines)
        })
        .await;

        // Handle timeout or stream result
        match result {
            Ok(Ok(lines)) => Ok(lines),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timed out waiting for 5 lines",
            ))),
        }
    }
}
