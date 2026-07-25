mod mock_server;
pub mod task_runner;
mod test_client;
mod test_server;
use async_trait::async_trait;
pub use mock_server::*;

use reqwest::{Client, Response, Url};
use rust_mcp_macros::{mcp_tool, JsonSchema};
use rust_mcp_schema::ProtocolVersion;
use rust_mcp_sdk::auth::{AuthInfo, AuthenticationError, OauthTokenVerifier};
use rust_mcp_sdk::mcp_client::ClientHandler;
use rust_mcp_sdk::mcp_icon;
use rust_mcp_sdk::schema::{ClientCapabilities, Implementation, InitializeRequestParams};
use std::collections::HashMap;
use std::sync::Once;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tracing_subscriber::EnvFilter;
use wiremock::{MockServer, Request, ResponseTemplate};

pub use test_client::*;
pub use test_server::*;

pub const NPX_SERVER_EVERYTHING: &str = "@modelcontextprotocol/server-everything";
pub const ONE_MILLISECOND: Option<Duration> = Some(Duration::from_millis(1));

#[cfg(unix)]
pub const UVX_SERVER_GIT: &str = "mcp-server-git";
static INIT: Once = Once::new();

pub fn init_tracing() {
    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("tracing"))
            .unwrap();

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init()
            .ok();
    });
}
#[mcp_tool(
    name = "say_hello",
    description = "Accepts a person's name and says a personalized \"Hello\" to that person",
    title = "A tool that says hello!",
    idempotent_hint = false,
    destructive_hint = false,
    open_world_hint = false,
    read_only_hint = false,
    meta = r#"{"version": "1.0"}"#
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SayHelloTool {
    /// The name of the person to greet with a "Hello".
    name: String,
}

pub async fn send_post_request(
    base_url: &str,
    message: &str,
    session_id: Option<&str>,
    post_headers: Option<HashMap<&str, &str>>,
) -> Result<Response, reqwest::Error> {
    let client = Client::new();
    let url = Url::parse(base_url).expect("Invalid URL");

    let mut headers = reqwest::header::HeaderMap::new();

    let protocol_version = ProtocolVersion::V2025_06_18.to_string();
    let post_headers = post_headers.unwrap_or({
        let mut map: HashMap<&str, &str> = HashMap::new();
        map.insert("Content-Type", "application/json");
        map.insert("Accept", "application/json, text/event-stream");
        map.insert("mcp-protocol-version", protocol_version.as_str());
        map
    });

    if let Some(sid) = session_id {
        headers.insert("mcp-session-id", sid.parse().unwrap());
    }

    for (key, value) in post_headers {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }

    let body = message.to_string();

    client.post(url).headers(headers).body(body).send().await
}

pub async fn send_delete_request(
    base_url: &str,
    session_id: Option<&str>,
    post_headers: Option<HashMap<&str, &str>>,
) -> Result<Response, reqwest::Error> {
    let client = Client::new();
    let url = Url::parse(base_url).expect("Invalid URL");

    let mut headers = reqwest::header::HeaderMap::new();

    let protocol_version = ProtocolVersion::V2025_06_18.to_string();
    let post_headers = post_headers.unwrap_or({
        let mut map: HashMap<&str, &str> = HashMap::new();
        map.insert("Content-Type", "application/json");
        map.insert("Accept", "application/json, text/event-stream");
        map.insert("mcp-protocol-version", protocol_version.as_str());
        map
    });

    if let Some(sid) = session_id {
        headers.insert("mcp-session-id", sid.parse().unwrap());
    }

    for (key, value) in post_headers {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }

    client.delete(url).headers(headers).send().await
}

pub async fn send_get_request(
    base_url: &str,
    extra_headers: Option<HashMap<&str, &str>>,
) -> Result<Response, reqwest::Error> {
    let client = Client::new();
    let url = Url::parse(base_url).expect("Invalid URL");

    let mut headers = reqwest::header::HeaderMap::new();

    if let Some(extra) = extra_headers {
        for (key, value) in extra {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
    }

    client.get(url).headers(headers).send().await
}

pub async fn send_option_request(
    base_url: &str,
    extra_headers: Option<HashMap<&str, &str>>,
) -> Result<Response, reqwest::Error> {
    let client = Client::new();
    let url = Url::parse(base_url).expect("Invalid URL");

    let mut headers = reqwest::header::HeaderMap::new();

    if let Some(extra) = extra_headers {
        for (key, value) in extra {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
    }

    client
        .request(reqwest::Method::OPTIONS, url)
        .headers(headers)
        .send()
        .await
}

use futures::stream::Stream;

// stream: &mut impl Stream<Item = Result<hyper::body::Bytes, hyper::Error>>,
/// reads sse events and return them as (id, event, data) tuple
pub async fn read_sse_event_from_stream(
    stream: &mut (impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin),
    event_count: usize,
) -> Option<Vec<(Option<String>, Option<String>, String)>> {
    let mut buffer = String::new();
    let mut events = vec![];

    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                let chunk_str = std::str::from_utf8(&chunk).unwrap();
                buffer.push_str(chunk_str);

                while let Some(pos) = buffer.find("\n\n") {
                    let (event_str, rest) = buffer.split_at(pos);
                    let mut id = None;
                    let mut event = None;
                    let mut data = None;

                    // Process the event string
                    for line in event_str.lines() {
                        if line.starts_with("id:") {
                            id = Some(line.trim_start_matches("id:").trim().to_string());
                        } else if line.starts_with("event:") {
                            event = Some(line.trim_start_matches("event:").trim().to_string());
                        } else if line.starts_with("data:") {
                            data = Some(line.trim_start_matches("data:").trim().to_string());
                        }
                    }

                    // Update buffer after processing
                    buffer = rest[2..].to_string(); // Skip "\n\n"

                    // Only include events with data
                    if let Some(data) = data {
                        events.push((id, event, data));
                        if events.len().eq(&event_count) {
                            return Some(events);
                        }
                    }
                }
            }
            Err(_e) => {
                return None;
            }
        }
    }
    if !events.is_empty() {
        Some(events)
    } else {
        None
    }
}

/// return sse event as (id, event, data) tuple
pub async fn read_sse_event(
    response: Response,
    event_count: usize,
) -> Option<Vec<(Option<String>, Option<String>, String)>> {
    let mut stream = response.bytes_stream();
    let events = read_sse_event_from_stream(&mut stream, event_count).await;
    events
}

pub fn test_client_info() -> InitializeRequestParams {
    InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "test-rust-mcp-client".into(),
            version: "0.1.0".into(),
            description: None,
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "dark"
            )],
            title: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V2025_03_26.to_string(),
        meta: None,
    }
}

pub struct TestClientHandler;

#[async_trait]
impl ClientHandler for TestClientHandler {}

pub fn sse_event(sse_raw: &str) -> String {
    sse_raw.replace("event: ", "")
}

pub fn sse_data(sse_raw: &str) -> String {
    sse_raw.replace("data: ", "")
}

/// Picks a free ephemeral port by binding to port 0 and reading the OS-assigned port.
///
/// The port is released immediately after discovery. Because `TcpListener::bind` creates
/// a listening socket (not a connection), no `TIME_WAIT` state is involved — the port
/// is immediately reusable by the server.
pub fn random_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr().map(|a| a.port()))
        .expect("random_port: failed to bind to ephemeral port")
}

pub mod sample_tools {
    use std::{sync::Arc, time::Duration};

    use rust_mcp_macros::{mcp_tool, JsonSchema};
    use rust_mcp_schema::{LoggingMessageNotificationParams, TextContent};
    use rust_mcp_sdk::{
        schema::{schema_utils::CallToolError, CallToolResult},
        McpServer,
    };
    use serde_json::json;

    //****************//
    //  TaskAugmentedTool  //
    //****************//
    #[mcp_tool(
        name = "task_augmented_tool",
        description = "invokes a long running mcp task, the result will be available after task is finished",
        idempotent_hint = false,
        destructive_hint = false,
        open_world_hint = false,
        read_only_hint = false
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, rust_mcp_macros::JsonSchema)]
    pub struct TaskAugmentedTool {
        /// TaskJobInfo indicating final status of the task and task duratins.
        pub job_info: TaskJobInfo,
    }

    //****************//
    //  SayHelloTool  //
    //****************//
    #[mcp_tool(
        name = "say_hello",
        description = "Accepts a person's name and says a personalized \"Hello\" to that person",
        idempotent_hint = false,
        destructive_hint = false,
        open_world_hint = false,
        read_only_hint = false
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, rust_mcp_macros::JsonSchema)]
    pub struct SayHelloTool {
        /// The name of the person to greet with a "Hello".
        pub name: String,
    }

    impl SayHelloTool {
        pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
            let hello_message = format!("Hello, {}!", self.name);

            return Ok(CallToolResult::text_content(vec![hello_message.into()]));
        }
    }

    //******************//
    //  AuthInfo Tool   //
    //******************//
    #[mcp_tool(
        name = "display_auth_info",
        description = "Displays auth_info if user is authenticated",
        idempotent_hint = false,
        destructive_hint = false,
        open_world_hint = false,
        read_only_hint = false
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct DisplayAuthInfo {}
    use rust_mcp_sdk::auth::AuthInfo;

    use crate::common::task_runner::TaskJobInfo;
    impl DisplayAuthInfo {
        pub fn call_tool(
            &self,
            auth_info: Option<AuthInfo>,
        ) -> Result<CallToolResult, CallToolError> {
            let message = format!("{}", serde_json::to_string(&auth_info).unwrap());

            return Ok(CallToolResult::text_content(vec![message.into()]));
        }
    }

    //******************//
    //  SayGoodbyeTool  //
    //******************//
    #[mcp_tool(
        name = "say_goodbye",
        description = "Accepts a person's name and says a personalized \"Goodbye\" to that person.",
        idempotent_hint = false,
        destructive_hint = false,
        open_world_hint = false,
        read_only_hint = false
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct SayGoodbyeTool {
        /// The name of the person to say goodbye to.
        name: String,
    }
    impl SayGoodbyeTool {
        pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
            let goodbye_message = format!("Goodbye, {}!", self.name);

            return Ok(CallToolResult::text_content(vec![goodbye_message.into()]));
        }
    }

    //****************************//
    //  StartNotificationStream   //
    //****************************//
    #[mcp_tool(
        name = "start-notification-stream",
        description = "Accepts a person's name and says a personalized \"Goodbye\" to that person."
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct StartNotificationStream {
        /// Interval in milliseconds between notifications
        interval: u64,
        /// Number of notifications to send (0 for 100)
        count: u32,
    }
    impl StartNotificationStream {
        pub async fn call_tool(
            &self,
            runtime: Arc<dyn McpServer>,
        ) -> Result<CallToolResult, CallToolError> {
            for i in 0..self.count {
                let _ = runtime
                    .send_logging_message(LoggingMessageNotificationParams {
                        data: json!({"id":format!("message {} of {}",i,self.count)}),
                        level: rust_mcp_sdk::schema::LoggingLevel::Emergency,
                        logger: None,
                        meta: None,
                    })
                    .await;
                tokio::time::sleep(Duration::from_millis(self.interval)).await;
            }

            let message = "so many messages sent".to_string();
            Ok(CallToolResult::text_content(vec![TextContent::from(
                message,
            )]))
        }
    }
}

pub async fn wiremock_request(mock_server: &MockServer, index: usize) -> Request {
    let requests = mock_server.received_requests().await.unwrap();
    requests[index].clone()
}

pub async fn debug_wiremock(mock_server: &MockServer) {
    let requests = mock_server.received_requests().await.unwrap();
    let len = requests.len();

    for (index, request) in requests.iter().enumerate() {
        println!("\n--- #{index} of {len} ---");
        println!("Method: {}", request.method);
        println!("Path: {}", request.url.path());
        // println!("Headers: {:#?}", request.headers);
        println!("---- headers ----");
        for (key, values) in &request.headers {
            println!("{key}: {values:?}");
        }

        let body_str = String::from_utf8_lossy(&request.body);
        println!("Body: {body_str}\n");
    }
}

pub fn create_sse_response(payload: &str) -> ResponseTemplate {
    let sse_body = format!(r#"data: {}{}"#, payload, "\n\n");
    ResponseTemplate::new(200).set_body_raw(sse_body.into_bytes(), "text/event-stream")
}

pub async fn wait_for_n_requests(
    mock_server: &MockServer,
    num_requests: usize,
    duration: Option<Duration>,
) {
    let duration = duration.unwrap_or(Duration::from_secs(1));
    timeout(duration, async {
        loop {
            let requests = mock_server.received_requests().await.unwrap();
            if requests.len() >= num_requests {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
}

pub struct TestTokenVerifier {
    token_map: HashMap<String, AuthInfo>,
}

impl TestTokenVerifier {
    pub fn new(token_map: HashMap<String, AuthInfo>) -> Self {
        Self { token_map }
    }
}

#[async_trait]
impl OauthTokenVerifier for TestTokenVerifier {
    async fn verify_token(&self, access_token: String) -> Result<AuthInfo, AuthenticationError> {
        let info = self.token_map.get(&access_token);

        let Some(info) = info else {
            return Err(AuthenticationError::InactiveToken);
        };

        if info.expires_at.unwrap() < SystemTime::now() {
            return Err(AuthenticationError::InvalidOrExpiredToken(
                "expired".to_string(),
            ));
        }

        Ok(info.clone())
    }
}
