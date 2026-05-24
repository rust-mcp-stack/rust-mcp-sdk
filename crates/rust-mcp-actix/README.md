# rust-mcp-actix

Actix-web HTTP server integration for [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk).

## Features

- **Turnkey server**: `create_actix_server().start().await`
- **BYO-server**: `mcp_scope()` to mount MCP endpoints on any existing Actix app
- **Streamable HTTP** + **SSE** transports
- **Authentication** (OAuth via `AuthProvider`)
- **Health check** endpoint
- **Session management** with `ActixRuntime`
- **Framework-agnostic** via `McpHttpServer` trait

## Quick Start

### Turnkey

```rust
use rust_mcp_actix::{create_actix_server, ActixServerOptions};

let server = create_actix_server(server_details, handler, ActixServerOptions::default());
server.start().await?;
```

### BYO-server (mount on existing Actix app)

```rust
use rust_mcp_actix::{mcp_scope, ActixMountOptions};

let mount = ActixMountOptions {
    streamable_http_endpoint: "/mcp".into(),
    sse_endpoint: "/sse".into(),
    sse_messages_endpoint: "/messages".into(),
    health_endpoint: Some("/health".into()),
};

HttpServer::new(move || {
    App::new()
        .service(web::scope("/api").route("", web::get().to(my_handler)))
        .service(rust_mcp_actix::mcp_scope(state.clone(), handler.clone(), &mount))
})
.bind("127.0.0.1:8080")?
.run()
.await?;
```

## Cargo Features

No feature flags. All functionality is always enabled since Actix has no optional TLS crate split.

## License

MIT
