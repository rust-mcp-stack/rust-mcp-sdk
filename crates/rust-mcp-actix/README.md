# rust-mcp-actix

[![crates.io](https://img.shields.io/crates/v/rust-mcp-actix?style=for-the-badge&logo=rust&color=FE965D)](https://crates.io/crates/rust-mcp-actix)
[![docs.rs](https://img.shields.io/badge/docs.rs-rust_mcp_actix-0ECDAB?style=for-the-badge&logo=docs.rs)](https://docs.rs/rust-mcp-actix)

Actix-web HTTP server integration for [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk).

Provides a production-ready Actix-web layer for MCP servers supporting **Streamable HTTP** and **SSE** transports. Choose this crate when you want to build on Actix-web or add MCP to an existing Actix application.

> **Prefer Axum?** See [`rust-mcp-axum`](https://crates.io/crates/rust-mcp-axum) for an equivalent integration built on Axum. Both crates expose the same feature set and follow the same usage patterns.

---

## Features

- **Turnkey server** — `create_actix_server().start().await`
- **BYO-server** — `mcp_scope()` + `McpMountOptions` to mount MCP endpoints on any existing Actix-web app
- **Streamable HTTP** + **SSE** transports (SSE enabled by default for backward compatibility)
- **Multi-client concurrency** with internal session management
- **Resumability** via pluggable `EventStore` (built-in `InMemoryEventStore`)
- **MCP Tasks** support via pluggable `TaskStore` (built-in `InMemoryTaskStore`)
- **OAuth authentication** via `AuthProvider`
- **DNS rebinding protection** (enabled by default)
- **HTTP health check** endpoint (optional)
- **TLS/SSL** support (via `ssl` cargo feature)
- **Custom session ID generators** and **session stores**
- **Message observer** hook for telemetry & monitoring

---

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
rust-mcp-sdk = "0.9"
rust-mcp-actix = "0.1"
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
```

### Turnkey Server

```rust
use rust_mcp_actix::{create_actix_server, ActixServerOptions};
use rust_mcp_sdk::{
    error::SdkResult,
    event_store::InMemoryEventStore,
    mcp_server::{ServerHandler, ToMcpServerHandler},
    schema::*,
};
use std::sync::Arc;

// Implement your handler ...
struct MyHandler;
#[async_trait::async_trait]
impl ServerHandler for MyHandler {}

#[tokio::main]
async fn main() -> SdkResult<()> {
    let server_details = InitializeResult { /* ... */ };

    let server = create_actix_server(
        server_details,
        MyHandler.to_mcp_server_handler(),
        ActixServerOptions {
            host: "127.0.0.1".to_string(),
            event_store: Some(Arc::new(InMemoryEventStore::default())), // enable resumability
            ..Default::default()
        },
    );
    server.start().await?;
    Ok(())
}
```

Once running, the server exposes:
- **Streamable HTTP** at `http://127.0.0.1:8080/mcp`
- **SSE** at `http://127.0.0.1:8080/sse` (for backward-compatible clients)

Test with [MCP Inspector](https://github.com/modelcontextprotocol/inspector):
```bash
npx -y @modelcontextprotocol/inspector@latest
# Then open: http://localhost:6274/?transport=streamable-http&serverUrl=http://localhost:8080/mcp
```

---

## BYO-server (Mount on an Existing Actix App)

Use `mcp_scope()` with `McpMountOptions` to integrate MCP into any existing Actix-web application:

```rust
use actix_web::{web, App, HttpServer};
use rust_mcp_actix::{mcp_scope, McpMountOptions};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler};
use rust_mcp_sdk::session_store::InMemorySessionStore;
use rust_mcp_sdk::id_generator::UuidGenerator;
use std::sync::Arc;

// Build the shared MCP state
let state = Arc::new(McpAppState {
    session_store: Arc::new(InMemorySessionStore::new()),
    id_generator: Arc::new(UuidGenerator {}),
    server_details: Arc::new(server_details),
    handler: my_handler.to_mcp_server_handler(),
    // ... other fields
});

let http_handler = Arc::new(McpHttpHandler::new(None, vec![], None));

let mount = McpMountOptions {
    streamable_http_endpoint: "/mcp".into(),
    sse_endpoint: "/sse".into(),
    sse_messages_endpoint: "/messages".into(),
    health_endpoint: Some("/health".into()),
    ..Default::default()
};

HttpServer::new(move || {
    App::new()
        // Your own routes
        .service(web::scope("/api").route("", web::get().to(my_handler_fn)))
        // MCP routes mounted under the same server
        .service(mcp_scope(state.clone(), http_handler.clone(), &mount))
})
.bind("127.0.0.1:8080")?
.run()
.await?;
```

👉 See the full working example: [`examples/byo-server.rs`](examples/byo-server.rs)

---

## `ActixServerOptions` Reference

All fields are optional except `host` and `port` (which have defaults). Use `..Default::default()` for any fields you don't need.

| Field | Type | Default | Description |
|---|---|---|---|
| `host` | `String` | `"127.0.0.1"` | Bind address |
| `port` | `u16` | `8080` | TCP port |
| `sse_support` | `bool` | `true` | Enable SSE transport for backward compat |
| `event_store` | `Option<Arc<dyn EventStore>>` | `None` | Enables resumability |
| `task_store` | `Option<Arc<ServerTaskStore>>` | `None` | Handles server-side MCP tasks |
| `client_task_store` | `Option<Arc<ClientTaskStore>>` | `None` | Tracks client-side MCP tasks |
| `session_store` | `Option<Arc<dyn SessionStore>>` | `None` (bounded in-memory) | Custom session backend |
| `session_id_generator` | `Option<Arc<dyn IdGenerator<SessionId>>>` | `None` (UUID) | Custom session ID strategy |
| `auth` | `Option<Arc<dyn AuthProvider>>` | `None` | OAuth authentication provider |
| `health_endpoint` | `Option<String>` | `None` (disabled) | Path for health check, e.g. `"/health"` |
| `health_handler` | `Option<Arc<dyn HealthHandler>>` | `None` (200 OK) | Custom health response handler |
| `message_observer` | `Option<Arc<dyn McpObserver<...>>>` | `None` | Telemetry / monitoring hook |
| `dns_rebinding` | `DnsRebindingOptions` | enabled | DNS rebinding protection config |
| `ping_interval` | `Duration` | 12 seconds | Keep-alive ping frequency |
| `enable_json_response` | `Option<bool>` | `false` | Return JSON instead of SSE stream |
| `max_request_body_size` | `Option<usize>` | 4 MiB | Maximum request body size |
| `enable_ssl` | `bool` | `false` | Enable TLS (requires `ssl` feature) |
| `ssl_cert_path` | `Option<String>` | `None` | Path to PEM certificate file |
| `ssl_key_path` | `Option<String>` | `None` | Path to PEM private key file |
| `custom_streamable_http_endpoint` | `Option<String>` | `None` (`/mcp`) | Override Streamable HTTP path |
| `custom_sse_endpoint` | `Option<String>` | `None` (`/sse`) | Override SSE path |
| `custom_messages_endpoint` | `Option<String>` | `None` (`/messages`) | Override SSE messages path |
| `transport_options` | `Arc<TransportOptions>` | default | Shared transport config |

### Full Example with Common Options

```rust
use rust_mcp_actix::{create_actix_server, ActixServerOptions};
use rust_mcp_sdk::{
    event_store::InMemoryEventStore,
    task_store::InMemoryTaskStore,
};
use std::sync::Arc;

let server = create_actix_server(
    server_details,
    handler.to_mcp_server_handler(),
    ActixServerOptions {
        host: "127.0.0.1".to_string(),
        port: 8080,
        event_store: Some(Arc::new(InMemoryEventStore::default())), // resumability
        task_store: Some(Arc::new(InMemoryTaskStore::new(None))),   // server tasks
        client_task_store: Some(Arc::new(InMemoryTaskStore::new(None))), // client tasks
        auth: Some(Arc::new(my_auth_provider)),                     // OAuth
        health_endpoint: Some("/health".into()),                    // health check
        sse_support: false,                                         // disable SSE if not needed
        ..Default::default()
    },
);
server.start().await?;
```

---

## Cargo Features

| Feature | Description |
|---|---|
| `ssl` | Enables TLS/SSL via Actix-web + `rustls`. Requires `ssl_cert_path` and `ssl_key_path` in options. |

> **Note:** Unlike `rust-mcp-axum`, Actix-web has no optional crypto provider split, so there is no `tls-no-provider` variant.

```toml
# With TLS/SSL
rust-mcp-actix = { version = "0.1", features = ["ssl"] }
```

---

## Security Considerations

When using Streamable HTTP transport, follow these best practices:

- **DNS rebinding protection** is enabled by default. If `allowed_hosts` is not set, it is auto-derived from `host:port`. For wildcard binds (`0.0.0.0`, `::`), explicitly configure `allowed_hosts` in `DnsRebindingOptions`.
- In local development, bind only to `127.0.0.1` rather than `0.0.0.0`.
- Use TLS/HTTPS for any production or internet-facing deployment (enable the `ssl` feature).

---

## Examples

| Example | Description |
|---|---|
| [`hello-world-server.rs`](examples/hello-world-server.rs) | Minimal turnkey Actix MCP server |
| [`byo-server.rs`](examples/byo-server.rs) | Mount MCP on an existing Actix-web app via `mcp_scope()` |

Run them with:
```bash
cargo run -p rust-mcp-actix --example hello-world-server
cargo run -p rust-mcp-actix --example byo-server
```

---

## License

MIT

---

<img align="top" src="assets/rust-mcp-stack-icon.png" width="24" style="border-radius:0.2rem;"> Part of the [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) ecosystem — a high-performance, asynchronous toolkit for building MCP servers and clients in Rust.
