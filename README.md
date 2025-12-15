<p align="center">
  <img width="200" src="assets/rust-mcp-sdk.png" alt="Description" width="300">
</p>

# Rust MCP SDK

[<img alt="crates.io" src="https://img.shields.io/crates/v/rust-mcp-sdk?style=for-the-badge&logo=rust&color=FE965D" height="22">](https://crates.io/crates/rust-mcp-sdk)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-rust_mcp_SDK-0ECDAB?style=for-the-badge&logo=docs.rs" height="22">](https://docs.rs/rust-mcp-sdk)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/rust-mcp-stack/rust-mcp-sdk/ci.yml?style=for-the-badge" height="22">
](https://github.com/rust-mcp-stack/rust-mcp-sdk/actions/workflows/ci.yml)
[<img alt="Hello World MCP Server" src="https://img.shields.io/badge/Example-Hello%20World%20MCP-0286ba?style=for-the-badge&logo=rust" height="22">
](examples/hello-world-mcp-server-stdio)


A high-performance, asynchronous Rust toolkit for building MCP servers and clients.
Focus on your application logic - rust-mcp-sdk handles the protocol, transports, and the rest!
This SDK fully implements the latest MCP protocol version ([2025-11-25](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema)), with backward compatibility built-in. `rust-mcp-sdk` provides the necessary components for developing both servers and clients in the MCP ecosystem. It leverages the [rust-mcp-schema](https://crates.io/crates/rust-mcp-schema) crate for type-safe schema objects and includes powerful procedural macros for tools and user input elicitation.


**Key Features**
- ✅ Transports:Stdio, Streamable HTTP, and backward-compatible SSE support
- ✅ Lightweight Axum-based server for Streamable HTTP and SSE
- ✅ Multi-client concurrency
- ✅ DNS Rebinding Protection
- ✅ Resumability
- ✅ Batch Messages
- ✅ Streaming & non-streaming JSON response
- ✅ OAuth Authentication for MCP Servers
  - ✅ [Remote Oauth Provider](crates/rust-mcp-sdk/src/auth/auth_provider/remote_auth_provider.rs) (for any provider with DCR support)
    - ✅ **Keycloak** Provider (via [rust-mcp-extra](crates/rust-mcp-extra/README.md#keycloak))
    - ✅ **WorkOS** Authkit Provider (via [rust-mcp-extra](crates/rust-mcp-extra/README.md#workos-authkit))
    - ✅ **Scalekit** Authkit Provider (via [rust-mcp-extra](crates/rust-mcp-extra/README.md#scalekit))
- ⬜ OAuth Authentication for MCP Clients

**⚠️** Project is currently under development and should be used at your own risk.

## Table of Contents
- [Quick Start](#quick-start)
  - [Minimal MCP Server (Stdio)](#minimal-mcp-server-stdio)
  - [Minimal MCP Server (Streamable HTTP)](#minimal-mcp-server-streamable-http)
  - [Minimal MCP Client (Stdio)](#minimal-mcp-server-stdio)
- [Usage Examples](#usage-examples)
  - [MCP Server (stdio)](#mcp-server-stdio)
  - [MCP Server (Streamable HTTP)](#mcp-server-streamable-http)
  - [MCP Client (stdio)](#mcp-client-stdio)
  - [MCP Client (Streamable HTTP)](#mcp-client-streamable-http)
  - [MCP Client (sse)](#mcp-client-sse)
- [Authentication](#authentication)
- [Macros](#macros)
- [HyperServerOptions](#hyperserveroptions)
  - [Security Considerations](#security-considerations)
- [Cargo features](#cargo-features)
  -  [Available Features](#available-features)
    - [MCP protocol versions with corresponding features](#mcp-protocol-versions-with-corresponding-features)
  -  [Default Features](#default-features)
  -  [Using Only the server Features](#using-only-the-server-features)
  -  [Using Only the client Features](#using-only-the-client-features)
- [Handler Traits](#handlers-traits)
  - [Choosing Between **ServerHandler** and **ServerHandlerCore**](#choosing-between-serverhandler-and-serverhandlercore)
  - [Choosing Between **ClientHandler** and **ClientHandlerCore**](#choosing-between-clienthandler-and-clienthandlercore)
- [Projects using Rust MCP SDK](#projects-using-rust-mcp-sdk)
- [Contributing](#contributing)
- [Development](#development)
- [License](#license)





## Quick Start

<!-- x-release-please-start-version -->

Add to your Cargo.toml:
```toml
[dependencies]
rust-mcp-sdk = "0.9.0"  # Check crates.io for the latest version
```
<!-- x-release-please-end -->


## Minimal MCP Server (Stdio)
```rs
use async_trait::async_trait;
use rust_mcp_sdk::{*,error::SdkResult,macros,mcp_server::{server_runtime, ServerHandler},schema::*,};

// Define a mcp tool
#[macros::mcp_tool(name = "say_hello", description = "returns \"Hello from Rust MCP SDK!\" message ")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, macros::JsonSchema)]
pub struct SayHelloTool {}

// define a custom handler
#[derive(Default)]
struct HelloHandler;

// implement ServerHandler
#[async_trait]
impl ServerHandler for HelloHandler {
    // Handles requests to list available tools.
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![SayHelloTool::tool()],
            meta: None,
            next_cursor: None,
        })
    }
    // Handles requests to call a specific tool.
    async fn handle_call_tool_request(&self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        if params.name == "say_hello" {
            Ok(CallToolResult::text_content(vec!["Hello from Rust MCP SDK!".into()]))
        } else {
            Err(CallToolError::unknown_tool(params.name))
        }
    }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Define server details and capabilities
    let server_info = InitializeResult {
        server_info: Implementation {
            name: "hello-rust-mcp".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Server".into()),
            description: Some("A minimal Rust MCP server".into()),
            icons: vec![mcp_icon!(src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "light")],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ServerCapabilities { tools: Some(ServerCapabilitiesTools { list_changed: None }), ..Default::default() },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        instructions: None,
        meta:None
    };

    let transport = StdioTransport::new(TransportOptions::default())?;
    let handler = HelloHandler::default().to_mcp_server_handler();
    let server = server_runtime::create_server(server_info, transport, handler);
    server.start().await
}
```

## Minimal MCP Server (Streamable HTTP)
Creating an MCP server in `rust-mcp-sdk` allows multiple clients to connect simultaneously with no additional setup.
The setup is nearly identical to the stdio example shown above. You only need to create a Hyper server via `hyper_server::create_server()` and pass in the same handler and `HyperServerOptions`.
💡 If backward compatibility is required, you can enable **SSE** transport by setting `sse_support` to true in `HyperServerOptions`.

```rust
use async_trait::async_trait;
use rust_mcp_sdk::{*,error::SdkResult,event_store::InMemoryEventStore,macros,
    mcp_server::{hyper_server, HyperServerOptions, ServerHandler},schema::*,    
};

// Define a mcp tool
#[macros::mcp_tool(
    name = "say_hello",
    description = "returns \"Hello from Rust MCP SDK!\" message "
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, macros::JsonSchema)]
pub struct SayHelloTool {}

// define a custom handler
#[derive(Default)]
struct HelloHandler;

// implement ServerHandler
#[async_trait]
impl ServerHandler for HelloHandler {
    // Handles requests to list available tools.
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {tools: vec![SayHelloTool::tool()],meta: None,next_cursor: None})
    }
    // Handles requests to call a specific tool.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        if params.name == "say_hello" {Ok(CallToolResult::text_content(vec!["Hello from Rust MCP SDK!".into()]))
        } else {
            Err(CallToolError::unknown_tool(params.name))
        }
    }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Define server details and capabilities
    let server_info = InitializeResult {
        server_info: Implementation {
            name: "hello-rust-mcp".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Server".into()),
            description: Some("A minimal Rust MCP server".into()),
            icons: vec![mcp_icon!(src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "light")],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ServerCapabilities { tools: Some(ServerCapabilitiesTools { list_changed: None }), ..Default::default() },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        instructions: None,
        meta:None
    };

    let handler = HelloHandler::default().to_mcp_server_handler();
    let server = hyper_server::create_server(
        server_info,
        handler,
        HyperServerOptions {
            host: "127.0.0.1".to_string(),
            event_store: Some(std::sync::Arc::new(InMemoryEventStore::default())), // enable resumability
            ..Default::default()
        },
    );
    server.start().await?;
    Ok(())
}
```


## Minimal MCP Client (Stdio)
Following is implementation of an MCP client that starts the [@modelcontextprotocol/server-everything](https://www.npmjs.com/package/@modelcontextprotocol/server-everything) server, displays the server's name, version, and list of tools provided by the server.


```rust
use async_trait::async_trait;
use rust_mcp_sdk::{*, error::SdkResult,
    mcp_client::{client_runtime, ClientHandler},
    schema::*,
};

// Custom Handler to handle incoming MCP Messages
pub struct MyClientHandler;
#[async_trait]
impl ClientHandler for MyClientHandler {
    // To see all the trait methods you can override,
    // check out:
    // https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler.rs
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Client details and capabilities
    let client_details: InitializeRequestParams = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "simple-rust-mcp-client".into(),
            version: "0.1.0".into(),
            description: None,
            icons: vec![],
            title: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        meta: None,
    };

    //  Create a transport, with options to launch @modelcontextprotocol/server-everything MCP Server
    let transport = StdioTransport::create_with_server_launch(
        "npx",vec!["-y".to_string(),"@modelcontextprotocol/server-everything@latest".to_string()],
        None,
        TransportOptions::default(),
    )?;

    // instantiate our custom handler for handling MCP messages
    let handler = MyClientHandler {};

    // Create and start the MCP client
    let client = client_runtime::create_client(client_details, transport, handler);    
    client.clone().start().await?;

    // use client methods to communicate with the MCP Server as you wish:

    let server_version = client.server_version().unwrap();    
    
    // Retrieve and display the list of tools available on the server
    let tools = client.request_tool_list(None).await?.tools;
    println!( "List of tools for {}@{}",server_version.name, server_version.version);
    tools.iter().enumerate().for_each(|(tool_index, tool)| {
        println!("  {}. {} : {}", tool_index + 1, tool.name, tool.description.clone().unwrap_or_default());
    });

    client.shut_down().await?;
    Ok(())
}
```

## Usage Examples

👉 For full examples (stdio, Streamable HTTP, clients, auth, etc.), see the [examples/](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples) directory.

👉 If you are looking for a step-by-step tutorial on how to get started with `rust-mcp-sdk` , please see : [Getting Started MCP Server](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/doc/getting-started-mcp-server.md)  

See [hello-world-mcp-server-stdio](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-stdio) example running in [MCP Inspector](https://modelcontextprotocol.io/docs/tools/inspector) :

![mcp-server in rust](assets/examples/hello-world-mcp-server.gif)


## Macros
Enable with the `macros` feature.  

[rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) includes several helpful macros that simplify common tasks when building MCP servers and clients. For example, they can automatically generate tool specifications and tool schemas right from your structs, or assist with elicitation requests and responses making them completely type safe. 

### ◾`mcp_tool`
Generate a [Tool](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Tool.html) from a struct, with rich metadata (icons, execution hints, etc.).

example usage:
```rs
#[mcp_tool(
   name = "write_file",
   title = "Write File Tool",
   description = "Create a new file or completely overwrite an existing file with new content.",
   destructive_hint = false idempotent_hint = false open_world_hint = false read_only_hint = false,
   meta = r#"{ "key" : "value", "string_meta" : "meta value", "numeric_meta" : 15}"#,
   execution(task_support = "optional"),
   icons = [(src = "https:/website.com/write.png", mime_type = "image/png", sizes = ["128x128"], theme = "light")]
)]
#[derive(rust_mcp_macros::JsonSchema)]
pub struct WriteFileTool {
    /// The target file's path for writing content.
    pub path: String,
    /// The string content to be written to the file
    pub content: String,
}
```

📝 For complete documentation, example usage, and a list of all available attributes, please refer to https://crates.io/crates/rust-mcp-macros.

### ◾ `tool_box!()` 
Automatically generates an enum based on the provided list of tools, making it easier to organize and manage them, especially when your application includes a large number of tools.

```rs
tool_box!(GreetingTools, [SayHelloTool, SayGoodbyeTool]);

let tools: Vec<Tool> = GreetingTools::tools();
``

💻 For a real-world example, check out [tools/](https://github.com/rust-mcp-stack/rust-mcp-filesystem/tree/main/src/tools) and 
[handle_call_tool_request(...)](https://github.com/rust-mcp-stack/rust-mcp-filesystem/blob/main/src/handler.rs#L195) in [rust-mcp-filesystem](https://github.com/rust-mcp-stack/rust-mcp-filesystem) project 

### ◾ [mcp_elicit](https://crates.io/crates/rust-mcp-macros)
Generates type-safe elicitation (Form or URL mode) for user input.

example usage:
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

// Sends a request to the client asking the user to provide input
let result: ElicitResult = server.request_elicitation(UserInfo::elicit_request_params()).await?;

// Convert result.content into a UserInfo instance
let user_info = UserInfo::from_elicit_result_content(result.content)?; 

println!("name: {}", user_info.name);
println!("age: {}", user_info.age);
println!("email: {}",user.email.clone().unwrap_or("not provider".into()));
println!("tags: {}", user_info.tags.join(",")); 
```
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
MCP server can verify tokens issued by other systems, integrate with external identity providers, or manage the entire authentication process itself. Each option offers a different balance of simplicity, security, and control.

 ### RemoteAuthProvider
  [RemoteAuthProvider](src/mcp_http/auth/auth_provider/remote_auth_provider.rs) RemoteAuthProvider enables authentication with identity providers that support Dynamic Client Registration (DCR) such as KeyCloak and WorkOS AuthKit, letting MCP clients auto-register and obtain credentials without manual setup.
  
👉 See the [server-oauth-remote](examples/auth/server-oauth-remote) example for how to use RemoteAuthProvider with a DCR-capable remote provider. 

👉 [rust-mcp-extra](https://crates.io/crates/rust-mcp-extra) also offers drop-in auth providers for common identity platforms, working seamlessly with rust-mcp-sdk:
 - [Keycloack auth example](crates/rust-mcp-extra/README.md#keycloak)
 - [WorkOS autn example](crates/rust-mcp-extra/README.md#workos-authkit)
 

 ### OAuthProxy  
 OAuthProxy enables authentication with OAuth providers that don’t support Dynamic Client Registration (DCR).It accepts any client registration request, handles the DCR on your server side and then uses your pre-registered app credentials upstream.The proxy also forwards callbacks, allowing dynamic redirect URIs to work with providers that require fixed ones.
 
> ⚠️ OAuthProxy support is still in development, please use RemoteAuthProvider for now.



## HyperServerOptions

HyperServer is a lightweight Axum-based server that streamlines MCP servers by supporting **Streamable HTTP** and **SSE** transports. It supports simultaneous client connections, internal session management, and includes built-in security features like DNS rebinding protection and more.

HyperServer is highly customizable through HyperServerOptions provided during initialization.

A typical example of creating a HyperServer that exposes the MCP server via Streamable HTTP and SSE transports at:

```rs

let server = hyper_server::create_server(
    server_details,
    handler.to_mcp_server_handler(),
    HyperServerOptions {
        host: "127.0.0.1".to_string(),
        port: 8080,
        event_store: Some(std::sync::Arc::new(InMemoryEventStore::default())), // enable resumability
        auth: Some(Arc::new(auth_provider)), // enable authentication
        sse_support: false,
        ..Default::default()
    },
);

server.start().await?;
```

📝 Refer to [HyperServerOptions](https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/hyper_servers/server.rs#L43) for a complete overview of HyperServerOptions attributes and options.


### Security Considerations

When using Streamable HTTP transport, following security best practices are recommended:

- Enable DNS rebinding protection and provide proper `allowed_hosts` and `allowed_origins` to prevent DNS rebinding attacks.
- When running locally, bind only to localhost (127.0.0.1 / localhost) rather than all network interfaces (0.0.0.0)
- Use TLS/HTTPS for production deployments


## Cargo Features

The `rust-mcp-sdk` crate provides several features that can be enabled or disabled. By default, all features are enabled to ensure maximum functionality, but you can customize which ones to include based on your project's requirements.

### Available Features

- `server`: Activates MCP server capabilities in `rust-mcp-sdk`, providing modules and APIs for building and managing MCP servers.
- `client`: Activates MCP client capabilities, offering modules and APIs for client development and communicating with MCP servers.
- `hyper-server`: This feature is necessary to enable `Streamable HTTP` or `Server-Sent Events (SSE)` transports for MCP servers. It must be used alongside the server feature to support the required server functionalities.
- `ssl`: This feature enables TLS/SSL support for the `Streamable HTTP` or `Server-Sent Events (SSE)` transport when used with the `hyper-server`.
- `macros`: Provides procedural macros for simplifying the creation and manipulation of MCP Tool structures.
- `sse`: Enables support for the `Server-Sent Events (SSE)` transport.
- `streamable-http`: Enables support for the `Streamable HTTP` transport.
- `stdio`: Enables support for the `standard input/output (stdio)` transport.
- `tls-no-provider`: Enables TLS without a crypto provider. This is useful if you are already using a different crypto provider than the aws-lc default.


### Default Features

When you add rust-mcp-sdk as a dependency without specifying any features, all features are enabled by default

<!-- x-release-please-start-version -->

```toml
[dependencies]
rust-mcp-sdk = "0.9.0"
```

<!-- x-release-please-end -->

### Using Only the server Features

If you only need the MCP Server functionality, you can disable the default features and explicitly enable the server feature. Add the following to your Cargo.toml:

<!-- x-release-please-start-version -->

```toml
[dependencies]
rust-mcp-sdk = { version = "0.2.0", default-features = false, features = ["server","macros","stdio"] }
```
Optionally add `hyper-server` and `streamable-http` for **Streamable HTTP** transport, and `ssl` feature for tls/ssl support of the `hyper-server`

<!-- x-release-please-end -->

### Using Only the client Features

If you only need the MCP Client functionality, you can disable the default features and explicitly enable the client feature.
Add the following to your Cargo.toml:

<!-- x-release-please-start-version -->

```toml
[dependencies]
rust-mcp-sdk = { version = "0.2.0", default-features = false, features = ["client","2024_11_05","stdio"] }
```

<!-- x-release-please-end -->

## Choosing Between Standard and Core Handlers traits
Learn when to use the  `mcp_*_handler` traits versus the lower-level `mcp_*_handler_core` traits for both server and client implementations. This section helps you decide based on your project's need for simplicity versus fine-grained control.

### Choosing Between `ServerHandler` and `ServerHandlerCore`

[rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) provides two type of handler traits that you can chose from:

- **ServerHandler**: This is the recommended trait for your MCP project, offering a default implementation for all types of MCP messages. It includes predefined implementations within the trait, such as handling initialization or responding to ping requests, so you only need to override and customize the handler functions relevant to your specific needs.
  Refer to [examples/hello-world-mcp-server-stdio/src/handler.rs](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-stdio/src/handler.rs) for an example.

- **ServerHandlerCore**: If you need more control over MCP messages, consider using `ServerHandlerCore`. It offers three primary methods to manage the three MCP message types: `request`, `notification`, and `error`. While still providing type-safe objects in these methods, it allows you to determine how to handle each message based on its type and parameters.
  Refer to [examples/hello-world-mcp-server-stdio-core/src/handler.rs](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-stdio-core/src/handler.rs) for an example.

---

**👉 Note:** Depending on whether you choose `ServerHandler` or `ServerHandlerCore`, you must use the `create_server()` function from the appropriate module:

- For `ServerHandler`:
  - Use `server_runtime::create_server()` for servers with stdio transport
  - Use `hyper_server::create_server()` for servers with sse transport

- For `ServerHandlerCore`:
  - Use `server_runtime_core::create_server()` for servers with stdio transport
  - Use `hyper_server_core::create_server()` for servers with sse transport

---


### Choosing Between `ClientHandler` and `ClientHandlerCore`

The same principles outlined above apply to the client-side handlers, `ClientHandler` and `ClientHandlerCore`.

- Use `client_runtime::create_client()` when working with `ClientHandler`

- Use `client_runtime_core::create_client()` when working with `ClientHandlerCore`

Both functions create an MCP client instance.



Check out the corresponding examples at: [examples/simple-mcp-client-stdio](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client-stdio) and [examples/simple-mcp-client-stdio-core](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client-stdio-core).


## Projects using Rust MCP SDK

Below is a list of projects that utilize the `rust-mcp-sdk`, showcasing their name, description, and links to their repositories or project pages.

|  | Name | Description | Link |
|------|------|-------------|------|
| <a href="https://rust-mcp-stack.github.io/rust-mcp-filesystem"><img src="https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-filesystem/refs/heads/main/docs/_media/rust-mcp-filesystem.png" width="64"/></a> | [Rust MCP Filesystem](https://rust-mcp-stack.github.io/rust-mcp-filesystem) | Fast, async MCP server enabling high-performance, modern filesystem operations with advanced features. | [GitHub](https://github.com/rust-mcp-stack/rust-mcp-filesystem) |
| <a href="https://rust-mcp-stack.github.io/mcp-discovery"><img src="https://raw.githubusercontent.com/rust-mcp-stack/mcp-discovery/refs/heads/main/docs/_media/mcp-discovery-logo.png" width="64"/></a> | [MCP Discovery](https://rust-mcp-stack.github.io/mcp-discovery) | A lightweight command-line tool for discovering and documenting MCP Server capabilities. | [GitHub](https://github.com/rust-mcp-stack/mcp-discovery) |
| <a href="https://github.com/EricLBuehler/mistral.rs"><img src="https://avatars.githubusercontent.com/u/65165915?s=64" width="64"/></a> | [mistral.rs](https://github.com/EricLBuehler/mistral.rs) | Blazingly fast LLM inference. | [GitHub](https://github.com/EricLBuehler/mistral.rs) |
| <a href="https://github.com/moonrepo/moon"><img src="https://avatars.githubusercontent.com/u/102833400?s=64" width="64"/></a> | [moon](https://github.com/moonrepo/moon) | moon is a repository management, organization, orchestration, and notification tool for the web ecosystem, written in Rust. | [GitHub](https://github.com/moonrepo/moon) |
| <a href="https://github.com/angreal/angreal"><img src="https://avatars.githubusercontent.com/u/45580675?s=64" width="64"/></a> | [angreal](https://github.com/angreal/angreal) | Angreal provides a way to template the structure of projects and a way of executing methods for interacting with that project in a consistent manner. | [GitHub](https://github.com/angreal/angreal) |
| <a href="https://github.com/FalkorDB/text-to-cypher"><img src="https://avatars.githubusercontent.com/u/140048192?s=64" width="64"/></a> | [text-to-cypher](https://github.com/FalkorDB/text-to-cypher) | A high-performance Rust-based API service that translates natural language text to Cypher queries for graph databases. | [GitHub](https://github.com/FalkorDB/text-to-cypher) |
| <a href="https://github.com/Tuurlijk/notify-mcp"><img src="https://avatars.githubusercontent.com/u/790979?s=64" width="64"/></a> | [notify-mcp](https://github.com/Tuurlijk/notify-mcp) | A Model Context Protocol (MCP) server that provides desktop notification functionality. | [GitHub](https://github.com/Tuurlijk/notify-mcp) |
| <a href="https://github.com/WismutHansen/lst"><img src="https://avatars.githubusercontent.com/u/86825018?s=64" width="64"/></a> | [lst](https://github.com/WismutHansen/lst) | `lst` is a personal lists, notes, and blog posts management application with a focus on plain-text storage, offline-first functionality, and multi-device synchronization. | [GitHub](https://github.com/WismutHansen/lst) |
| <a href="https://github.com/Vaiz/rust-mcp-server"><img src="https://avatars.githubusercontent.com/u/4908982?s=64" width="64"/></a> | [rust-mcp-server](https://github.com/Vaiz/rust-mcp-server) | `rust-mcp-server` allows the model to perform actions on your behalf, such as building, testing, and analyzing your Rust code. | [GitHub](https://github.com/Vaiz/rust-mcp-server) |









## Contributing

We welcome everyone who wishes to contribute! Please refer to the [contributing](CONTRIBUTING.md) guidelines for more details.

Check out our [development guide](development.md) for instructions on setting up, building, testing, formatting, and trying out example projects.

All contributions, including issues and pull requests, must follow
Rust's Code of Conduct.

Unless explicitly stated otherwise, any contribution you submit for inclusion in rust-mcp-sdk is provided under the terms of the MIT License, without any additional conditions or restrictions.

## Development

Check out our [development guide](development.md) for instructions on setting up, building, testing, formatting, and trying out example projects.

## License

This project is licensed under the MIT License. see the [LICENSE](LICENSE) file for details.
