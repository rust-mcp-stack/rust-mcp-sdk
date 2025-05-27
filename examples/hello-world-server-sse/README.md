# Hello World MCP Server - SSE Transport

A basic MCP server implementation using SSE transport, featuring two custom tools: `Say Hello` and `Say Goodbye` , utilizing [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) , using SSE transport

## Overview

This project showcases a fundamental MCP server implementation, highlighting the capabilities of [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) with these features:

- SSE transport
- Custom server handler
- Basic server capabilities

## Running the Example

1. Clone the repository:

```bash
git clone git@github.com:rust-mcp-stack/rust-mcp-sdk.git
cd rust-mcp-sdk
```

2. Build and start the server:

```bash
cargo run -p hello-world-server-sse --release
```

By default, the SSE endpoint is accessible at `http://127.0.0.1:8080/sse`.
You can test it with [MCP Inspector](https://modelcontextprotocol.io/docs/tools/inspector), or alternatively, use it with any MCP client you prefer.

Here you can see it in action :

![hello-world-mcp-server](../../assets/examples/hello-world-server-sse.gif)
