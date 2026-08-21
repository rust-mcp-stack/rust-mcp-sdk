# Building a Custom HTTP Framework Backend

While `rust-mcp-sdk` provides turnkey integrations for **Axum** (`rust-mcp-axum`) and **Actix** (`rust-mcp-actix`), the SDK's HTTP layer is fundamentally framework-agnostic. 

You can build an integration for any Rust HTTP framework (like **Rocket**, **Warp**, **Salvo**, or even **Hyper** directly) by bridging your framework's native Request/Response types to the SDK's core HTTP handling logic.

This guide outlines the architectural contract and steps required to build a custom HTTP backend.

## 1. Architectural Overview

At its core, `rust-mcp-sdk` handles HTTP transports via the `McpHttpServer` and an associated `McpAppState`. 

The SDK operates exclusively on types from the standard [`http`](https://crates.io/crates/http) crate (i.e., `http::Request`, `http::Response`, `http::HeaderMap`) and standard `bytes::Bytes` for payload bodies. 

To integrate a new framework, your job is simply to **translate** between your framework's native types and the standard `http` types, and then pass the data to `McpAppState`.

## 2. The Shared State (`McpAppState`)

Your web server needs access to the MCP state. In Axum and Actix, we wrap `McpAppState` in an `Arc` and pass it to the route handlers. You must do the same for your framework.

`McpAppState` provides the primary entry point for processing MCP HTTP requests:
```rust
// Defined in `rust-mcp-sdk::mcp_http::McpAppState`
pub async fn handle_mcp_request(
    &self,
    req_parts: http::request::Parts,
    req_body: bytes::Bytes,
) -> Result<http::Response<http_body_util::Full<bytes::Bytes>>, rust_mcp_sdk::error::SdkError>
```

## 3. Required Endpoints

To fully support both the modern **Streamable HTTP** transport and the backward-compatible **SSE (Server-Sent Events)** transport, your framework must expose three specific routes:

### A. Streamable HTTP Endpoint
- **Method:** `POST`
- **Path:** `/mcp` (or your chosen mount point)
- **Action:** Accept the incoming JSON payload, pass it to `handle_mcp_request`, and stream the response back.

### B. SSE Connection Endpoint (Optional but recommended)
- **Method:** `GET`
- **Path:** `/sse` 
- **Action:** Accept the connection and return an infinite SSE stream. The SDK will handle formatting the events (including the `endpoint` URL event required by the spec).

### C. SSE Message Endpoint (Optional but recommended)
- **Method:** `POST`
- **Path:** `/mcp/messages` (The SDK dictates this relative to the SSE endpoint)
- **Action:** Accept messages from SSE clients and pass them to the server.

## 4. Step-by-Step Implementation Guide

### Step 1: Create the Server and State
First, you initialize the MCP Server exactly as you would with Stdio, but using `StreamableHttpTransport`.

```rust
let transport = StreamableHttpTransport::new(TransportOptions::default());
let (http_server, app_state) = McpHttpServer::new(
    transport.clone(), 
    /* sse_transport= */ None, 
    /* auth_provider= */ None
);

let server = server_runtime::create_server(server_details, transport, handler);
```

### Step 2: Spawn the Server Lifecycles
Both the `McpServer` and the `McpHttpServer` must be spawned as background Tokio tasks so they can process messages asynchronously while your web framework handles incoming HTTP requests.

```rust
// Start the MCP core server
tokio::spawn(async move {
    server.start().await.unwrap();
});

// Start the HTTP transport state machine
tokio::spawn(async move {
    http_server.start().await.unwrap();
});
```

### Step 3: Implement the Route Handlers (The Adapter)
In your custom framework's route handlers, you must extract the request, pass it to `app_state.handle_mcp_request()`, and convert the result back to your framework's response type.

Here is pseudo-code for what a Rocket or Salvo handler might look like:

```rust
async fn handle_mcp_post(
    state: State<Arc<McpAppState>>, 
    request: FrameworkRequest
) -> FrameworkResponse {
    // 1. Convert framework headers/URL to standard `http::request::Parts`
    let parts = convert_to_http_parts(&request);
    
    // 2. Extract the body as `bytes::Bytes`
    let body_bytes = request.body().await;

    // 3. Pass to the SDK
    match state.handle_mcp_request(parts, body_bytes).await {
        Ok(sdk_response) => {
            // 4. Convert standard `http::Response` back to your framework's response
            convert_to_framework_response(sdk_response)
        },
        Err(e) => {
            // Return 500 Internal Server Error
            FrameworkResponse::internal_server_error(e.to_string())
        }
    }
}
```

## 5. Reference Implementations

The best way to see exactly how to write these conversion functions and handle streaming bodies is to look at the source code of our existing integrations:

- **Axum:** [`crates/rust-mcp-axum/src/server.rs`](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-axum/src/server.rs)
- **Actix:** [`crates/rust-mcp-actix/src/server.rs`](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-actix/src/server.rs)
