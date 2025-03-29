# rust-mcp-transport.

`rust-mcp-transport` is a part of the [rust-mcp-sdk](https://crates.io/crates/rust-mcp-sdk) ecosystem, offering transport implementations for the MCP (Model Context Protocol). It enables asynchronous data exchange and efficient MCP message handling between MCP Clients and Servers.

**⚠️WARNING**: Currently, only Standard Input/Output (stdio) transport is supported. Server-Sent Events (SSE) transport is under development and will be available soon.

## Usage Example

### For MCP Server

```rust
use rust_mcp_transport::{StdioTransport, TransportOptions};

// create a stdio transport to be used in a MCP Server
let transport = StdioTransport::new(TransportOptions { timeout: 60_000 })?;

```

Refer to the [Hello World MCP Server](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server) example for a complete demonstration.

### For MCP Client

```rust
use rust_mcp_transport::{StdioTransport, TransportOptions};

// create a stdio transport that launches `server-everything` MCP Server
let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".to_string(), "@modelcontextprotocol/server-everything"],
        None,
        TransportOptions { timeout: 60_000 }
    )?;

```

With environment variables:

```rust
use rust_mcp_transport::{StdioTransport, TransportOptions};

// environment variables will be available to the MCP server at launch time
let environment_value = HashMap::from([(
       "API_KEY".to_string(),
       "A1B2C3D4E5F6G7H8I9J0K1L2M3N4O5P6".to_string(),
   )]);

// configure an arbitrary MCP Server to launch with argument and environment variables
let transport = StdioTransport::create_with_server_launch(
    "your-mcp-server",
    vec!["argument".to_string()],
    Some(environment_value),
    TransportOptions::default(),
)?;
```

Refer to the [Simple MCP Client](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client) example for a complete demonstration.

---

<img align="top" src="assets/rust-mcp-stack-icon.png" width="24" style="border-radius:0.2rem;"> Check out [rust-mcp-sdk](https://crates.io/crates/rust-mcp-sdk) , a high-performance, asynchronous toolkit for building MCP servers and clients. Focus on your app's logic while [rust-mcp-sdk](https://crates.io/crates/rust-mcp-sdk) takes care of the rest!

---
