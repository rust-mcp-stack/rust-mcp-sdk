# Simple MCP Client (SSE)

This is a simple MCP (Model Context Protocol) client implemented with the rust-mcp-sdk, dmeonstrating SSE transport, showcasing fundamental MCP client operations like fetching the MCP server's capabilities and executing a tool call.

## Overview

This project demonstrates a basic MCP client implementation, showcasing the features of the [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk).

This example connects to a running instance of the [@modelcontextprotocol/server-everything](https://www.npmjs.com/package/@modelcontextprotocol/server-everything) server, which has already been started with the sse flag.

It displays the server name and version, outlines the server's capabilities, and provides a list of available tools, prompts, templates, resources, and more offered by the server. Additionally, it will execute a tool call by utilizing the add tool from the server-everything package to sum two numbers and output the result.

> Note that @modelcontextprotocol/server-everything is an npm package, so you must have Node.js and npm installed on your system, as this example attempts to start it.

## Running the Example

1. Clone the repository:

```bash
git clone git@github.com:rust-mcp-stack/rust-mcp-sdk.git
cd rust-mcp-sdk
```

2- Start `@modelcontextprotocol/server-everything` with SSE argument:

```bash
npx @modelcontextprotocol/server-everything sse
```

> It launches the server, making everything accessible via the SSE transport at http://localhost:3001/sse.

2. Open a new terminal and run the project with:

```bash
cargo run -p simple-mcp-client-sse
```

You can observe a sample output of the project; however, your results may vary slightly depending on the version of the MCP Server in use when you run it.

<img src="../../assets/examples/simple-mcp-client-sse.png" width="640"/>
