#[cfg(feature = "hyper-server")]
pub mod test_server_common {
    use async_trait::async_trait;
    use tokio_stream::StreamExt;

    use rust_mcp_schema::{
        Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
        LATEST_PROTOCOL_VERSION,
    };
    use rust_mcp_sdk::{
        mcp_server::{
            hyper_server, HyperServer, HyperServerOptions, IdGenerator, ServerHandler,
            ServerHandlerCore,
        },
        McpServer, SessionId,
    };
    use std::sync::RwLock;
    use std::time::Duration;
    use tokio::time::timeout;

    pub const INITIALIZE_REQUEST: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2.0","capabilities":{"sampling":{},"roots":{"listChanged":true}},"clientInfo":{"name":"reqwest-test","version":"0.1.0"}}}"#;
    pub const PING_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;

    pub fn test_server_details() -> InitializeResult {
        InitializeResult {
            // server name and version
            server_info: Implementation {
                name: "Test MCP Server".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: ServerCapabilities {
                // indicates that server support mcp tools
                tools: Some(ServerCapabilitiesTools { list_changed: None }),
                ..Default::default() // Using default values for other fields
            },
            meta: None,
            instructions: Some("server instructions...".to_string()),
            protocol_version: "2025-03-26".to_string(),
        }
    }

    pub struct TestServerHandler;

    #[async_trait]
    impl ServerHandler for TestServerHandler {
        async fn on_server_started(&self, runtime: &dyn McpServer) {
            let _ = runtime
                .stderr_message("Server started successfully".into())
                .await;
        }
    }

    pub fn create_test_server(options: HyperServerOptions) -> HyperServer {
        hyper_server::create_server(test_server_details(), TestServerHandler {}, options)
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

    impl IdGenerator for TestIdGenerator {
        fn generate(&self) -> SessionId {
            let mut lock = self.generated.write().unwrap();
            *lock += 1;
            if *lock > self.constant_ids.len() {
                *lock = 1;
            }
            self.constant_ids[*lock - 1].to_owned()
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
