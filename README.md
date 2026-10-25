<p align="center">
  <img width="200" src="https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-sdk.png" alt="Rust MCP SDK" width="300">
</p>


<div align="center">

[<img alt="crates.io" src="https://img.shields.io/crates/v/rust-mcp-sdk?style=for-the-badge&logo=rust&color=FE965D" height="22">](https://crates.io/crates/rust-mcp-sdk)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-rust_mcp_SDK-0ECDAB?style=for-the-badge&logo=docs.rs" height="22">](https://docs.rs/rust-mcp-sdk)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/rust-mcp-stack/rust-mcp-sdk/ci.yml?style=for-the-badge" height="22">](https://github.com/rust-mcp-stack/rust-mcp-sdk/actions/workflows/ci.yml)
[<img alt="conformance" src="https://img.shields.io/badge/conformance-2026--07--28%20100%25-green?style=for-the-badge" height="22">](https://github.com/rust-mcp-stack/rust-mcp-sdk/actions/workflows/conformance.yml)
[<img alt="tier-check" src="https://img.shields.io/github/actions/workflow/status/rust-mcp-stack/rust-mcp-sdk/tier-check.yml?style=for-the-badge" height="22">](https://github.com/rust-mcp-stack/rust-mcp-sdk/actions/workflows/tier-check.yml)

A high-performance, asynchronous Rust toolkit for building MCP servers and clients.

[Documentation](https://rust-mcp-stack.github.io/rust-mcp-sdk/) · [Tutorials](https://rust-mcp-stack.github.io/rust-mcp-sdk/docs/tutorials/build-your-first-mcp-server) · [Examples](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/crates/rust-mcp-sdk/examples) · [Upgrade Guide](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/UPGRADING.md) · [Changelog](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/CHANGELOG.md) <br/>
[Contributing](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/CONTRIBUTING.md) · [Report a bug](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/new?template=bug_report.md) · [Request a Feature](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/new?template=feature_request.md)

</div>

------

**🥇 Tier 1 SDK:** passes 100% of scored conformance scenarios, and the official MCP conformance tool's `tier-check` reports **Tier 1** under SEP-1730. [Tier-check runs](https://github.com/rust-mcp-stack/rust-mcp-sdk/actions/workflows/tier-check.yml) · [release reports](https://github.com/rust-mcp-stack/rust-mcp-sdk/releases)

✅ This SDK fully implements the [MCP 2026-07-28](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/) stateless protocol and passes **100% of official MCP conformance tests**.

`rust-mcp-sdk` provides the necessary components for developing both servers and clients in the MCP ecosystem. It leverages the [rust-mcp-schema](https://crates.io/crates/rust-mcp-schema) crate for type-safe schema objects and includes powerful procedural macros.

Focus on your application logic, rust-mcp-sdk handles the protocol, transports, and the rest.

----

**Version notice:** This is `rust-mcp-sdk` **2.x**, implementing the new **MCP 2026-07-28** (stateless) specification.
 For the previous **MCP 2025-11-25** specification, use **`rust-mcp-sdk` 1.x**.
 See the [upgrade guide](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/UPGRADING.md) to migrate from 1.x to 2.0.

**Version matrix:**
| SDK version | Protocol | Branch |
|---|---|---|
| 2.0 | MCP 2026-07-28 (stateless) | `main` |
| 1.x (LTS) | MCP 2025-11-25 | `release-1.x` |

**Upgrading?** See the [upgrade guide](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/UPGRADING.md).
----


**Key Features**
- ✅ Latest MCP protocol specification supported: 2026-07-28
- **🥇 Tier 1 SDK** - official MCP conformance tool's `tier-check` reports **Tier 1** under SEP-1730. [see tier-check runs](https://github.com/rust-mcp-stack/rust-mcp-sdk/actions/workflows/tier-check.yml)·
- ✅ **100% MCP Conformance** - passes all [official](https://github.com/modelcontextprotocol/conformance) client (254/254) and server (40/40) [conformance tests](https://github.com/rust-mcp-stack/rust-mcp-sdk/actions/workflows/conformance.yml)
- ✅ Transports: Stdio, Streamable HTTP, and backward-compatible SSE support
- ✅ Framework Agnostic: Axum, Actix, and BYO Server integrations
- ✅ MRTR (Mid-Request Turn-Around) for server→client input requests
- ✅ Response cache (SEP-2549) with principal-scoped privacy + auto-pagination
- ✅ Per-request `_meta` with `RequestContext`
- ✅ Multi-client concurrency
- ✅ DNS Rebinding Protection
- ✅ Message Observer (Telemetry & Monitoring)
- ✅ HTTP Health Checks (for load balancers & container orchestration)
- ✅ OAuth Authentication for MCP Servers
  - ✅ [Remote Oauth Provider](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/auth/auth_provider/remote_auth_provider.rs)
    - **Keycloak** Provider (via [rust-mcp-extra](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-extra/README.md#keycloak))
    - **WorkOS** Authkit Provider (via [rust-mcp-extra](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-extra/README.md#workos-authkit))
    - **Scalekit** Authkit Provider (via [rust-mcp-extra](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-extra/README.md#scalekit))
- ✅ OAuth Authentication for MCP Clients (metadata discovery, CIMD, PKCE, token refresh, pluggable storage)
- ✅ Issuer-bound credentials (SEP-2352), strict `iss` validation (SEP-2468)

## Table of Contents
- [Quick Start](#quick-start)
- [Minimal MCP Server (Stdio)](#minimal-mcp-server-stdio)
- [Minimal MCP Server (Streamable HTTP)](#minimal-mcp-server-streamable-http)
- [Minimal MCP Client (Stdio)](#minimal-mcp-client-stdio)
- [Usage Examples](#usage-examples)
- [Macros](#macros)
  - [mcp_tool](#mcp_tool)
  - [tool_box](#-tool_box)
  - [mcp_elicit](#-mcp_elicit)
  - [mcp_resource](#-mcp_resource)
  - [mcp_resource_template](#-mcp_resource_template)
  - [mcp_prompt](#-mcp_prompt)
  - [mcp_icon](#-mcp_icon)
- [Authentication](#authentication)
- [HTTP Server Backends (Axum & Actix)](#http-server-backends-axum--actix)
- [Cargo features](#cargo-features)
- [Handler Traits](#handler-traits)
- [Message Observer (Telemetry & Monitoring)](#message-observer-telemetry--monitoring)
- [Health Check Endpoint](#health-check-endpoint)
- [Contributing](#contributing)
- [Development](#development)
- [License](#license)


## Quick Start

<!-- x-release-please-start-version -->

Add to your Cargo.toml:
```toml
[dependencies]
rust-mcp-sdk = "2.0.0"  # Check crates.io for the latest version
```
<!-- x-release-please-end -->


## Minimal MCP Server (Stdio)
```rs
use async_trait::async_trait;
use rust_mcp_sdk::{
    error::SdkResult, macros, mcp_icon,
    mcp_server::{server_runtime, McpServerOptions, ServerHandler},
    schema::*,
    McpServer, RequestContext, ServerDetails, StdioTransport, ToMcpServerHandler,
    TransportOptions,
};

// Define an MCP tool
#[macros::mcp_tool(name = "say_hello", description = "returns \"Hello from Rust MCP SDK!\" message")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, macros::JsonSchema)]
pub struct SayHelloTool {}

// Define a custom handler
#[derive(Default)]
struct HelloHandler;

#[async_trait]
impl ServerHandler for HelloHandler {
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![SayHelloTool::tool()],
            meta: None,
            next_cursor: None,
            cache_scope: Default::default(),
            result_type: "complete".to_string(),
            ttl_ms: 0,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, CallToolError> {
        if params.name == "say_hello" {
            Ok(ServerResult::CallToolResult(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent::new(
                    "Hello from Rust MCP SDK!".to_string(),
                    None,
                    None,
                ))],
                is_error: None,
                meta: None,
                result_type: "complete".to_string(),
            }))
        } else {
            Err(CallToolError::unknown_tool(params.name))
        }
    }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    let server_details = ServerDetails {
        server_info: Implementation {
            name: "hello-rust-mcp".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Server".into()),
            description: Some("A minimal Rust MCP server".into()),
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "light"
            )],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        instructions: None,
        meta: None,
    };

    let transport = StdioTransport::new(TransportOptions::default())?;
    let handler = HelloHandler::default().to_mcp_server_handler();
    let server = server_runtime::create_server(McpServerOptions {
        server_details,
        transport,
        handler,
        message_observer: None,
    });
    server.start().await
}
```

## HTTP Server Backends (Axum & Actix)

Creating a Streamable HTTP MCP server in `rust-mcp-sdk` allows multiple clients to connect simultaneously with no additional setup. The setup is nearly identical to the stdio example , the only difference is which HTTP backend crate you install and which function you call to create the server.

Post only - the 2026-07-28 protocol is stateless. GET and DELETE endpoints return 405 Method Not Allowed.

### Axum Backend (`rust-mcp-axum`)

Add [`rust-mcp-axum`](https://crates.io/crates/rust-mcp-axum) to your dependencies and use `create_axum_server()` with `AxumServerOptions`.

```rust
use async_trait::async_trait;
use rust_mcp_axum::{create_axum_server, AxumServerOptions};
use rust_mcp_sdk::{
    error::SdkResult, macros,
    mcp_server::ServerHandler, schema::*,
};

// ... (define SayHelloTool and HelloHandler as shown above)

#[tokio::main]
async fn main() -> SdkResult<()> {
    let server_details = ServerDetails { /* ... */ };

    let handler = HelloHandler::default().to_mcp_server_handler();
    let server = create_axum_server(
        server_details,
        handler,
        AxumServerOptions {
            host: "127.0.0.1".to_string(),
            ..Default::default()
        },
    );
    server.start().await?;
    Ok(())
}
```

### Actix-web Backend (`rust-mcp-actix`)

Add [`rust-mcp-actix`](https://crates.io/crates/rust-mcp-actix) to your dependencies and use `create_actix_server()` with `ActixServerOptions`.

```rust
use rust_mcp_actix::{create_actix_server, ActixServerOptions};
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_server::ServerHandler, schema::*,
};

// ... (define SayHelloTool and HelloHandler as shown above)

#[tokio::main]
async fn main() -> SdkResult<()> {
    let server_details = ServerDetails { /* ... */ };

    let handler = HelloHandler::default().to_mcp_server_handler();
    let server = create_actix_server(
        server_details,
        handler,
        ActixServerOptions {
            host: "127.0.0.1".to_string(),
            ..Default::default()
        },
    );
    server.start().await?;
    Ok(())
}
```

### BYO-server: Embed MCP in your Existing App

Both backends support a **BYO-server** (Bring Your Own Server) mode, letting you mount MCP endpoints onto a router or app you already control - no need to hand over the server lifecycle.

| Backend | Function | Docs |
|---|---|---|
| Axum | `mcp_routes(state, &mount_opts, http_handler)` | [`rust-mcp-axum` README](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-axum/README.md) |
| Actix-web | `mcp_scope(state, http_handler, &mount_opts)` | [`rust-mcp-actix` README](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-actix/README.md) |

### Custom HTTP Framework Integrations

The SDK is completely framework-agnostic. If you are using a different HTTP framework (like Rocket, Salvo, or Warp), you can build a custom integration by adapting your framework's native Request/Response types to the SDK's core HTTP handling logic.

See the [Custom HTTP Framework Integration Guide](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/doc/custom-http-framework-integration.md) for architectural details.

### AxumServerOptions

Axum server is highly customizable through `AxumServerOptions`:

```rs
let server = create_axum_server(
    server_details,
    handler.to_mcp_server_handler(),
    AxumServerOptions {
        host: "127.0.0.1".to_string(),
        port: 8080,
        auth: Some(Arc::new(auth_provider)),           // enable authentication
        health_endpoint: Some("/health".into()),         // health check
        sse_support: true,                               // backward-compat SSE
        ..Default::default()
    },
);
server.start().await?;
```

### Security Considerations

- DNS rebinding protection is enabled by default. If `allowed_hosts` is not set, it auto-derives from `host:port`.
- When running locally, bind only to localhost (127.0.0.1 / localhost) rather than all network interfaces (0.0.0.0)
- Use TLS/HTTPS for production deployments

Following is implementation of an MCP client that starts the [@modelcontextprotocol/server-everything](https://www.npmjs.com/package/@modelcontextprotocol/server-everything) server, discovers the server's capabilities, lists available tools, and calls a tool.

```rust
use async_trait::async_trait;
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_client::{client_runtime, ClientHandler, McpClientOptions},
    schema::*,
    ClientDetails, McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
};

pub struct MyClientHandler;
#[async_trait]
impl ClientHandler for MyClientHandler {
    // Override handler methods as needed.
    // See: crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler.rs
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    let client_details = ClientDetails {
        client_info: Implementation {
            name: "simple-rust-mcp-client".into(),
            version: "0.1.0".into(),
            description: None,
            icons: vec![],
            title: None,
            website_url: None,
        },
        capabilities: ClientCapabilities::default(),
    };

    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".to_string(), "@modelcontextprotocol/server-everything@latest".to_string()],
        None,
        TransportOptions::default(),
    )?;

    let handler = MyClientHandler {};
    let client = client_runtime::create_client(McpClientOptions::new(
        client_details,
        transport,
        handler.to_mcp_client_handler(),
    ));
    client.clone().start().await?;

    // Discover the server
    let discover = client.request_discover(Default::default()).await?;
    println!("Supported protocol versions: {:?}", discover.supported_versions);

    // List tools
    let tools = client.request_tool_list(None).await?.tools;
    tools.iter().enumerate().for_each(|(i, tool)| {
        println!("  {}. {} : {}", i + 1, tool.name, tool.description.unwrap_or_default());
    });

    // Call a tool (supports MRTR auto-retry)
    let result = client.call_tool(CallToolRequestParams {
        name: "say_hello".to_string(),
        arguments: None,
        input_responses: None,
        request_state: None,
        meta: RequestMetaObject::default(),
    }).await?;

    client.shut_down().await?;
    Ok(())
}
```

## Usage Examples

For more examples (stdio, Streamable HTTP, clients, auth, etc.), see the [examples/](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/crates/rust-mcp-sdk/examples/) directory.

👉 For step-by-step tutorials (server, client, HTTP deployment, OAuth, and more), see the [documentation site](https://rust-mcp-stack.github.io/rust-mcp-sdk/docs/tutorials/build-your-first-mcp-server).

See the [hello-world-mcp-server-stdio](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/examples/hello-world-mcp-server-stdio.rs) example running in the [MCP Inspector](https://modelcontextprotocol.io/docs/tools/inspector):

<img src="https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/examples/hello-world-mcp-server.gif" alt="hello world mcp server in rust" width="800" />


## Macros
Enable with the `macros` feature.

### `mcp_tool`
Generate a [Tool](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Tool.html) from a struct, with metadata (icons, hints, etc.).

```rs
#[mcp_tool(
    name = "write_file",
    title = "Write File Tool",
    description = "Create or overwrite a file with new content.",
    destructive_hint = false, idempotent_hint = false, open_world_hint = false, read_only_hint = false,
    meta = r#"{ "key": "value" }"#,
    icons = [(src = "https://website.com/write.png", mime_type = "image/png", sizes = ["128x128"], theme = "light")]
)]
#[derive(rust_mcp_macros::JsonSchema)]
pub struct WriteFileTool {
    /// The target file's path for writing content.
    pub path: String,
    /// The string content to be written to the file
    pub content: String,
}
```

### `tool_box!()`
Automatically generates an enum based on the provided list of tools.

```rs
tool_box!(GreetingTools, [SayHelloTool, SayGoodbyeTool]);
let tools: Vec<Tool> = GreetingTools::tools();
```

### `mcp_elicit()`
Generates type-safe elicitation (Form or URL mode) for user input.

```rs
#[mcp_elicit(message = "Please enter your info", mode = form)]
#[derive(JsonSchema)]
pub struct UserInfo {
    #[json_schema(title = "Name", min_length = 5, max_length = 100)]
    pub name: String,
    #[json_schema(title = "Email", format = "email")]
    pub email: Option<String>,
    #[json_schema(title = "Age", minimum = 15, maximum = 125)]
    pub age: i32,
    #[json_schema(title = "Tags")]
    pub tags: Vec<String>,
}
```

### ◾ [mcp_resource()](https://crates.io/crates/rust-mcp-macros)
A procedural macro attribute that generates utility methods to create fully populated [Resource](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Resource.html) instances from compile-time metadata , usually used for exposing static assets like files, images, or documents. Also generates a `RESOURCE_URI` associated constant, usable in `match` patterns, and a `resource_mime_type()` accessor.

📝 For complete documentation, example usage, and a list of all available attributes, please refer to https://crates.io/crates/rust-mcp-macros.

 ### ◾ [mcp_resource_template()](https://crates.io/crates/rust-mcp-macros)
A procedural macro attribute that generates utility methods to create fully populated [ResourceTemplate](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.ResourceTemplate.html) instances from compile-time metadata for exposing parameterized server resources. Also generates a `RESOURCE_URI_TEMPLATE` associated constant, usable in `match` patterns, and a `resource_template_mime_type()` accessor.

📝 For complete documentation, example usage, and a list of all available attributes, please refer to https://crates.io/crates/rust-mcp-macros.

### ◾ [mcp_prompt()](https://crates.io/crates/rust-mcp-macros)
A procedural macro attribute that generates utility methods to create fully populated [Prompt](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Prompt.html) instances from compile-time metadata, and , when the optional `messages` attribute is provided , to parse request arguments (`from_arguments`) and render them into a `GetPromptResult` (`render`). Struct fields become typed prompt arguments (`String` = required, `Option<String>` = optional, `String` + `default` = fallback), the prompts/get handler itself is left to the user.

📝 For complete documentation, example usage, and a list of all available attributes, please refer to https://crates.io/crates/rust-mcp-macros.

### ◾ `mcp_icon!()`
A convenient icon builder for implementations and tools, offering full attribute support including theme, size, mime, and more.

example usage:
```rs
let icon: crate::schema::Icon = mcp_icon!(
    src = "http://website.com/icon.png",
    mime_type = "image/png",
    sizes = ["64x64"],
    theme = "dark"
);
```

## Authentication
MCP servers can verify tokens issued by other systems, integrate with external identity providers, or manage the entire authentication process.

### RemoteAuthProvider
[RemoteAuthProvider](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/auth/auth_provider/remote_auth_provider.rs) enables authentication with identity providers that support Dynamic Client Registration (DCR), letting MCP clients auto-register and obtain credentials.

### OAuthProxy
OAuthProxy enables authentication with OAuth providers that don't support DCR.

## Cargo Features

### Available Features

- `server`: Activates MCP server capabilities
- `client`: Activates MCP client capabilities
- `macros`: Procedural macros for Tool, Elicit, Resource structures
- `sse`: Server-Sent Events (SSE) transport
- `streamable-http`: Streamable HTTP transport
- `stdio`: Standard input/output (stdio) transport
- `auth`: OAuth authentication support for MCP servers
- `tls-no-provider`: TLS without a crypto provider

### Default Features

All features are enabled by default:

<!-- x-release-please-start-version -->

```toml
[dependencies]
rust-mcp-sdk = "2.0.0"
```
<!-- x-release-please-end -->

### Using Only the Server Features

<!-- x-release-please-start-version -->

```toml
[dependencies]
rust-mcp-sdk = { version = "2.0.0", default-features = false, features = ["server", "macros", "stdio"] }
```
<!-- x-release-please-end -->

### Using Only the Client Features

<!-- x-release-please-start-version -->

```toml
[dependencies]
rust-mcp-sdk = { version = "2.0.0", default-features = false, features = ["client", "stdio"] }
```
<!-- x-release-please-end -->

## Handler Traits

### Choosing Between `ServerHandler` and `ServerHandlerCore`

- **ServerHandler**: Recommended. Default implementations for all MCP messages. Override only what you need.
- **ServerHandlerCore**: Full control over request/notification/error dispatch.

**Note:** Use `server_runtime::create_server()` or `server_runtime_core::create_server()` depending on which handler you implement.

### Choosing Between `ClientHandler` and `ClientHandlerCore`

Same principles apply on the client side: use `client_runtime::create_client()` with `ClientHandler`, or `client_runtime_core::create_client()` with `ClientHandlerCore`.

## Message Observer (Telemetry & Monitoring)

Implement `McpObserver` to intercept all incoming and outgoing MCP messages for telemetry, logging, debugging, or monitoring.

```rs
let server = server_runtime::create_server(McpServerOptions {
    server_details,
    transport,
    handler: handler.to_mcp_server_handler(),
    message_observer: Some(SimpleServerObserver::new()),
});
```

## Health Check Endpoint

An optional HTTP health check endpoint for load balancers and container orchestration:

```rs
let server = create_axum_server(
    server_details,
    handler.to_mcp_server_handler(),
    AxumServerOptions {
        host: "127.0.0.1".into(),
        health_endpoint: Some("/health".into()),
        ..Default::default()
    },
);
```

## Contributing

We welcome everyone who wishes to contribute! Please refer to the [contributing](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/CONTRIBUTING.md) guidelines for more details.

Check out our [development guide](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/development.md) for instructions on setting up, building, testing, formatting, and trying out example projects.

All contributions, including issues and pull requests, must follow Rust's Code of Conduct.

Unless explicitly stated otherwise, any contribution you submit for inclusion in rust-mcp-sdk is provided under the terms of the MIT License, without any additional conditions or restrictions.

## Development

Check out our [development guide](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/development.md) for instructions on setting up, building, testing, formatting, and trying out example projects.

## License

This project is licensed under the MIT License. see the [LICENSE](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/LICENSE) file for details.
