# Hello World MCP Server (Core) - Streamable Http

A simple MCP server implementation with two custom tools  `Say Hello` and `Say Goodbye` , utilizing [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk). It uses Streamable HTTP as the primary transport, while also supporting SSE for backward compatibility.

## Overview

This project showcases a fundamental MCP server implementation, highlighting the capabilities of
[rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) with these features:

- Streamable HTTP transport
- SSE transport (for backward compatibility)
- Custom server handler
- Basic server capabilities

💡 By default, both **Streamable HTTP** and **SSE** transports are enabled for backward compatibility.
To disable the SSE transport, set the `sse_support` value in the `HyperServerOptions` accordingly:

```rs
let server =
    hyper_server_core::create_server(server_details, handler,
        HyperServerOptions{
            sse_support: false, // Disable SSE support
            Default::default()
        });
```


## Running the Example

1. Clone the repository:

```bash
git clone git@github.com:rust-mcp-stack/rust-mcp-sdk.git
cd rust-mcp-sdk
```

2. Build and start the server:

```bash
cargo run -p hello-world-server-streamable-http-core --release
```

By default, both the Streamable HTTP and SSE endpoints are displayed in the terminal:

```sh
• Streamable HTTP Server is available at http://127.0.0.1:8080/mcp
• SSE Server is available at http://127.0.0.1:8080/sse
```

You can test them out with [MCP Inspector](https://modelcontextprotocol.io/docs/tools/inspector), or alternatively, use it with any MCP client you prefer.

```bash
npx -y @modelcontextprotocol/inspector@latest
```

That will open the inspector in a browser,

Then , to test the server, visit one of the following URLs based on the desired transport:

* Streamable HTTP:
  [http://localhost:6274/?transport=streamable-http\&serverUrl=http://localhost:8080/mcp](http://localhost:6274/?transport=streamable-http&serverUrl=http://localhost:8080/mcp)
* SSE:
  [http://localhost:6274/?transport=sse\&serverUrl=http://localhost:8080/sse](http://localhost:6274/?transport=sse&serverUrl=http://localhost:8080/sse)


Here you can see it in action :

![hello-world-mcp-server-sse-core](../../assets/examples/hello-world-server-streamable-http-core.gif)
