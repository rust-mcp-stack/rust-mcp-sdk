use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{MessageFromServer, RequestFromServer},
    CreateTaskResult, ElicitRequestParams, ElicitResult, ElicitResultAction, ElicitResultContent,
    ElicitResultContentPrimitive, LoggingMessageNotificationParams, PingRequest, RequestParams,
    RpcError, TaskStatus, TaskStatusNotificationParams,
};
use rust_mcp_sdk::{
    mcp_client::ClientHandler,
    schema::{NotificationFromServer, ResultFromClient},
    task_store::{ClientTaskCreator, CreateTaskOptions},
    McpClient,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use crate::common::task_runner::{McpTaskRunner, TaskJobInfo};

#[cfg(feature = "hyper-server")]
pub mod test_client_common {
    use rust_mcp_schema::{
        schema_utils::MessageFromServer, ClientCapabilities, ClientElicitation, ClientRoots,
        ClientSampling, ClientTaskElicitation, ClientTaskRequest, ClientTaskSampling, ClientTasks,
        Implementation, InitializeRequestParams, LATEST_PROTOCOL_VERSION,
    };
    use rust_mcp_sdk::{
        mcp_client::{client_runtime, ClientRuntime},
        mcp_icon,
        task_store::InMemoryTaskStore,
        McpClient, RequestOptions, SessionId, StreamableTransportOptions,
    };
    use serde_json::Map;
    use std::{collections::HashMap, sync::Arc, time::Duration};
    use tokio::sync::RwLock;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    use wiremock::{
        matchers::{body_json_string, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::common::{
        create_sse_response, task_runner::McpTaskRunner, test_server_common::INITIALIZE_RESPONSE,
        wait_for_n_requests,
    };

    pub struct InitializedClient {
        pub client: Arc<ClientRuntime>,
        pub mcp_url: String,
        pub mock_server: MockServer,
    }

    pub const TEST_SESSION_ID: &str = "test-session-id";

    pub const INITIALIZE_REQUEST: &str = r#"{"id":0,"jsonrpc":"2.0","method":"initialize","params":{"capabilities":{"elicitation":{"form":{},"url":{}},"roots":{"listChanged":true},"sampling":{"context":{},"tools":{}},"tasks":{"cancel":{},"list":{},"requests":{"elicitation":{"create":{}},"sampling":{"createMessage":{}}}}},"clientInfo":{"icons":[{"mimeType":"image/png","sizes":["128x128"],"src":"https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png","theme":"dark"}],"name":"simple-rust-mcp-client-sse","title":"Simple Rust MCP Client (SSE)","version":"0.1.0"},"protocolVersion":"2025-11-25"}}"#;

    pub fn test_client_details() -> InitializeRequestParams {
        InitializeRequestParams {
            // capabilities: ClientCapabilities::default(),
            capabilities: {
                ClientCapabilities{
                    elicitation: Some(ClientElicitation{ form: Some(Map::new()), url: Some(Map::new()) }),
                    experimental: None,
                    roots: Some(ClientRoots{ list_changed: Some(true) }),
                    sampling: Some(ClientSampling{ context: Some(Map::new()), tools: Some(Map::new()) }),
                    tasks: Some(ClientTasks{ cancel:  Some(Map::new()), list:  Some(Map::new()),
                        requests: Some(ClientTaskRequest{ elicitation: Some(ClientTaskElicitation{ create:Some( Map::new() )}), sampling: Some(ClientTaskSampling{ create_message: Some( Map::new() ) }) }) }),
                }},
            client_info: Implementation {
                name: "simple-rust-mcp-client-sse".to_string(),
                version: "0.1.0".to_string(),
                title: Some("Simple Rust MCP Client (SSE)".to_string()),
                description: None,
                icons: vec![mcp_icon!(
                    src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                    mime_type = "image/png",
                    sizes = ["128x128"],
                    theme = "dark"
                )],
                website_url: None,
            },
            protocol_version: LATEST_PROTOCOL_VERSION.into(),
            meta: None,
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
            mcp_task_runner: McpTaskRunner::new(),
        };

        let client = client_runtime::with_transport_options(
            client_details,
            transport_options,
            handler,
            Some(Arc::new(InMemoryTaskStore::new(None))),
        );

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

// Test handler
pub struct TestClientHandler {
    message_history: Arc<RwLock<Vec<MessageFromServer>>>,
    mcp_task_runner: McpTaskRunner,
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
        params: Option<RequestParams>,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<rust_mcp_schema::Result, RpcError> {
        self.register_message(&MessageFromServer::RequestFromServer(
            RequestFromServer::PingRequest(params),
        ))
        .await;

        Ok(rust_mcp_schema::Result {
            meta: Some(json!({"meta_number":1515}).as_object().unwrap().to_owned()),
            extra: None,
        })
    }

    async fn handle_logging_message_notification(
        &self,
        params: LoggingMessageNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        self.register_message(&MessageFromServer::NotificationFromServer(
            NotificationFromServer::LoggingMessageNotification(params.clone()),
        ))
        .await;
        Ok(())
    }

    async fn handle_task_status_notification(
        &self,
        params: TaskStatusNotificationParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        self.register_message(&MessageFromServer::NotificationFromServer(
            NotificationFromServer::TaskStatusNotification(params.clone()),
        ))
        .await;
        Ok(())
    }

    async fn handle_task_augmented_elicit_request(
        &self,
        task_creator: ClientTaskCreator,
        params: ElicitRequestParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<CreateTaskResult, RpcError> {
        self.register_message(&MessageFromServer::RequestFromServer(
            RequestFromServer::ElicitRequest(params.clone()),
        ))
        .await;

        let ElicitRequestParams::FormParams(form_params) = params else {
            panic!("Expected a form elicitation!")
        };

        let task_store = runtime.task_store().unwrap();

        let task = task_creator
            .create_task(CreateTaskOptions {
                ttl: form_params.task.unwrap().ttl,
                poll_interval: None,
                meta: None,
            })
            .await;
        let mut content: HashMap<String, ElicitResultContent> = HashMap::new();
        content.insert(
            "content".to_string(),
            ElicitResultContentPrimitive::String("hello".to_string()).into(),
        );
        let result = ResultFromClient::ElicitResult(ElicitResult {
            action: ElicitResultAction::Accept,
            content: Some(content),
            meta: None,
        });
        let job_info: TaskJobInfo = TaskJobInfo {
            finish_in_ms: 300,
            status_interval_ms: 100,
            task_final_status: TaskStatus::Completed.to_string(),
            task_result: Some(serde_json::to_string(&result).unwrap()),
            meta: None,
        };
        let task = self
            .mcp_task_runner
            .run_client_task(task, task_store, job_info, runtime.session_id().await)
            .await;

        Ok(CreateTaskResult { meta: None, task })
    }
}
