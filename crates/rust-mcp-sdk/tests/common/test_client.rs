use async_trait::async_trait;
use rust_mcp_schema::{schema_utils::MessageFromServer, PingRequest, RpcError};
use rust_mcp_sdk::{mcp_client::ClientHandler, McpClient};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "hyper-server")]
pub mod test_client_common {
    use rust_mcp_schema::{
        schema_utils::MessageFromServer, ClientCapabilities, Implementation,
        InitializeRequestParams, LATEST_PROTOCOL_VERSION,
    };
    use rust_mcp_sdk::{
        mcp_client::{client_runtime, ClientRuntime},
        McpClient, RequestOptions, SessionId, StreamableTransportOptions,
    };
    use std::{collections::HashMap, sync::Arc, time::Duration};
    use tokio::sync::RwLock;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    use wiremock::{
        matchers::{body_json_string, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::common::{
        create_sse_response, test_server_common::INITIALIZE_RESPONSE, wait_for_n_requests,
    };

    pub struct InitializedClient {
        pub client: Arc<ClientRuntime>,
        pub mcp_url: String,
        pub mock_server: MockServer,
    }

    pub const TEST_SESSION_ID: &str = "test-session-id";
    pub const INITIALIZE_REQUEST: &str = r#"{"id":0,"jsonrpc":"2.0","method":"initialize","params":{"capabilities":{},"clientInfo":{"name":"simple-rust-mcp-client-sse","title":"Simple Rust MCP Client (SSE)","version":"0.1.0"},"protocolVersion":"2025-06-18"}}"#;

    pub fn test_client_details() -> InitializeRequestParams {
        InitializeRequestParams {
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "simple-rust-mcp-client-sse".to_string(),
                version: "0.1.0".to_string(),
                title: Some("Simple Rust MCP Client (SSE)".to_string()),
            },
            protocol_version: LATEST_PROTOCOL_VERSION.into(),
        }
    }

    pub async fn create_client(
        mcp_url: &str,
        custom_headers: Option<HashMap<String, String>>,
    ) -> (Arc<ClientRuntime>, Arc<RwLock<Vec<MessageFromServer>>>) {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        let client_details: InitializeRequestParams = test_client_details();

        let transport_options = StreamableTransportOptions {
            mcp_url: mcp_url.to_string(),
            request_options: RequestOptions {
                request_timeout: Duration::from_secs(2),
                custom_headers,
                ..RequestOptions::default()
            },
        };

        let message_history = Arc::new(RwLock::new(vec![]));
        let handler = super::TestClientHandler {
            message_history: message_history.clone(),
        };

        let client =
            client_runtime::with_transport_options(client_details, transport_options, handler);

        // client.clone().start().await.unwrap();
        (client, message_history)
    }

    pub async fn initialize_client(
        session_id: Option<SessionId>,
        custom_headers: Option<HashMap<String, String>>,
    ) -> InitializedClient {
        let mock_server = MockServer::start().await;

        // initialize response
        let mut response = create_sse_response(INITIALIZE_RESPONSE);

        if let Some(session_id) = session_id {
            response = response.append_header("mcp-session-id", session_id.as_str());
        }

        // initialize request and response
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_json_string(INITIALIZE_REQUEST))
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        // receive initialized notification
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_json_string(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            ))
            .respond_with(ResponseTemplate::new(202))
            .expect(1)
            .mount(&mock_server)
            .await;

        let mcp_url = format!("{}/mcp", mock_server.uri());
        let (client, _) = create_client(&mcp_url, custom_headers).await;

        client.clone().start().await.unwrap();

        wait_for_n_requests(&mock_server, 2, None).await;

        InitializedClient {
            client,
            mcp_url,
            mock_server,
        }
    }
}

// Custom responder for SSE with 10 ping messages
struct SsePingResponder;

// Test handler
pub struct TestClientHandler {
    message_history: Arc<RwLock<Vec<MessageFromServer>>>,
}

impl TestClientHandler {
    async fn register_message(&self, message: &MessageFromServer) {
        let mut lock = self.message_history.write().await;
        lock.push(message.clone());
    }
}

#[async_trait]
impl ClientHandler for TestClientHandler {
    async fn handle_ping_request(
        &self,
        request: PingRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<rust_mcp_schema::Result, RpcError> {
        self.register_message(&request.into()).await;

        Ok(rust_mcp_schema::Result {
            meta: Some(json!({"meta_number":1515}).as_object().unwrap().to_owned()),
            extra: None,
        })
    }
}
