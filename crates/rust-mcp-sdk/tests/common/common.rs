mod test_server;
use async_trait::async_trait;
use reqwest::{Client, Response, Url};
use rust_mcp_macros::{mcp_tool, JsonSchema};
use rust_mcp_schema::ProtocolVersion;
use rust_mcp_sdk::mcp_client::ClientHandler;

use rust_mcp_sdk::schema::{ClientCapabilities, Implementation, InitializeRequestParams};
use std::collections::HashMap;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_stream::StreamExt;

pub use test_server::*;

pub const NPX_SERVER_EVERYTHING: &str = "@modelcontextprotocol/server-everything";

#[cfg(unix)]
pub const UVX_SERVER_GIT: &str = "mcp-server-git";

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

use futures::stream::Stream;

// stream: &mut impl Stream<Item = Result<hyper::body::Bytes, hyper::Error>>,
pub async fn read_sse_event_from_stream(
    stream: &mut (impl Stream<Item = Result<hyper::body::Bytes, reqwest::Error>> + Unpin),
    event_count: usize,
) -> Option<Vec<String>> {
    let mut buffer = String::new();
    let mut events = vec![];

    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                let chunk_str = std::str::from_utf8(&chunk).unwrap();
                buffer.push_str(chunk_str);

                while let Some(pos) = buffer.find("\n\n") {
                    let data = {
                        // Scope to limit borrows
                        let (event_str, rest) = buffer.split_at(pos);
                        let mut data = None;

                        // Process the event string
                        for line in event_str.lines() {
                            if line.starts_with("data:") {
                                data = Some(line.trim_start_matches("data:").trim().to_string());
                                break; // Exit loop after finding data
                            }
                        }

                        // Update buffer after processing
                        buffer = rest[2..].to_string(); // Skip "\n\n"
                        data
                    };

                    // Return if data was found
                    if let Some(data) = data {
                        events.push(data);
                        if events.len().eq(&event_count) {
                            return Some(events);
                        }
                    }
                }
            }
            Err(_e) => {
                // return Err(TransportServerError::HyperError(e));
                return None;
            }
        }
    }
    None
}

pub async fn read_sse_event(response: Response, event_count: usize) -> Option<Vec<String>> {
    let mut stream = response.bytes_stream();
    read_sse_event_from_stream(&mut stream, event_count).await
}

pub fn test_client_info() -> InitializeRequestParams {
    InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "test-rust-mcp-client".into(),
            version: "0.1.0".into(),
            #[cfg(feature = "2025_06_18")]
            title: None,
        },
        protocol_version: ProtocolVersion::V2025_03_26.to_string(),
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

// Simple Xorshift PRNG struct
struct Xorshift {
    state: u64,
}

impl Xorshift {
    // Initialize with a seed based on system time and process ID
    fn new() -> Self {
        // Get nanoseconds since UNIX epoch
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time error")
            .as_nanos() as u64;
        // Get process ID for additional entropy
        let pid = process::id() as u64;
        // Combine nanos and pid with a simple mix
        let seed = nanos ^ (pid << 32) ^ (nanos.rotate_left(17));
        Xorshift { state: seed | 1 } // Ensure non-zero seed
    }

    // Generate the next random u64 using Xorshift
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    // Generate a random u16 within a range [min, max]
    fn next_u16_range(&mut self, min: u16, max: u16) -> u16 {
        assert!(max >= min, "max must be greater than or equal to min");
        let range = (max - min + 1) as u64;
        min + (self.next_u64() % range) as u16
    }
}

// Generate a random port number in the range [8081, 15000]
pub fn random_port() -> u16 {
    const MIN_PORT: u16 = 8081;
    const MAX_PORT: u16 = 15000;

    let mut rng = Xorshift::new();
    rng.next_u16_range(MIN_PORT, MAX_PORT)
}

pub fn random_port_old() -> u16 {
    let min: u16 = 8081;
    let max: u16 = 15000;
    let range = max - min + 1;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("systime error!");

    // Combine seconds and nanoseconds for better entropy
    let nanos = now.subsec_nanos() as u64;
    let secs = now.as_secs();

    // Simple hash-like mix
    let mixed = (nanos ^ (secs << 16)) ^ (nanos.rotate_left(13));

    min + ((mixed as u16) % range)
}

pub mod sample_tools {
    #[cfg(feature = "2025_06_18")]
    use rust_mcp_sdk::macros::{mcp_tool, JsonSchema};
    use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult};

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
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct SayHelloTool {
        /// The name of the person to greet with a "Hello".
        pub name: String,
    }

    impl SayHelloTool {
        pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
            let hello_message = format!("Hello, {}!", self.name);

            #[cfg(feature = "2025_06_18")]
            return Ok(CallToolResult::text_content(vec![
                rust_mcp_sdk::schema::TextContent::from(hello_message),
            ]));
            #[cfg(not(feature = "2025_06_18"))]
            return Ok(CallToolResult::text_content(hello_message, None));
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

            #[cfg(feature = "2025_06_18")]
            return Ok(CallToolResult::text_content(vec![
                rust_mcp_sdk::schema::TextContent::from(goodbye_message),
            ]));
            #[cfg(not(feature = "2025_06_18"))]
            return Ok(CallToolResult::text_content(goodbye_message, None));
        }
    }
}
