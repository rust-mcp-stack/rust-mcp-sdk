# Rust MCP SDK - Examples

This folder contains a variety of example programs demonstrating how to use the rust-mcp-sdk crate to build MCP clients and MCP servers. The examples cover different transports (stdio, streamable HTTP/SSE) and variations (handler vs. core handler implementations).

## List of Examples
- **MCP Server**
    -  **[stdio examples](#%EF%B8%8F-mcp-server-examples-stdio)**
        - quick-start-server-stdio
        -  hello-world-mcp-server-stdio
        -  hello-world-mcp-server-stdio-core
    - **[Streamable HTTP Examples](#%EF%B8%8F-mcp-servers-examples-streamable-http)**
        - quick-start-streamable-http
        - hello-world-server-streamable-http
        - hello-world-server-streamable-http-core        
    - **[Oauth Example](#%EF%B8%8F-mcp-server---oauth-example)**
        - mcp-server-oauth-remote
- **MCP Client**
    -  **[stdio examples](#%EF%B8%8F-mcp-client-examples-stdio)**
        - quick-start-client-stdio
        - simple-mcp-client-stdio
        - simple-mcp-client-stdio-core
    - **[Streamable HTTP Examples](#%EF%B8%8F-mcp-client-examples-streamable-http)**      
        - simple-mcp-client-streamable-http
        - simple-mcp-client-streamable-http-core
    - **[sse Examples](#%EF%B8%8F-mcp-client-examples-sse)**
        - simple-mcp-client-sse
        - simple-mcp-client-sse-core 

-----


### ➡️ MCP Server Examples (stdio)
Basic MCP server implementation using *stdio* transport, featuring two custom tools: `Say Hello` and `Say Goodbye`.
`hello-world-mcp-server-stdio` and `hello-world-mcp-server-stdio-core` also provides two static resource and a resource template that returns a Pokemon sprite as a blob resource.

- [quick-start-server-stdio.rs](quick-start-server-stdio.rs)
- [hello-world-mcp-server-stdio.rs](hello-world-mcp-server-stdio.rs)
- [hello-world-mcp-server-stdio-core.rs](hello-world-mcp-server-stdio-core.rs)

**Build the server:**
_for instance, build the `hello-world-mcp-server-stdio`_
```sh
cargo build --example hello-world-mcp-server-stdio # or quick_start_server_stdio or hello-world-mcp-server-stdio-core
```
The compiled binary will be located at `target/debug/examples/`

**Testing:**
You can use this binary with any MCP-compatible client. For easy testing and inspection, launch it in the [MCP Inspector](https://github.com/modelcontextprotocol/inspector), by selecting `stdio` transport , pointing it to the generated binary and connecting to the server.  

Here you can see it in action :
<img src="../assets/examples/hello-world-mcp-server.gif" alt="hello-world-mcp-server" width="800" />

-----

### ➡️ MCP Servers Examples (Streamable HTTP)
Minimal quick-start example demonstrating the fastest way to set up a basic MCP server using the **Streamable HTTP** transport and **SSE** for backward compatibility.

- [quick-start-streamable-http.rs](quick-start-streamable-http.rs)
- [hello-world-server-streamable-http.rs](hello-world-server-streamable-http.rs)
- [hello-world-server-streamable-http-core.rs](hello-world-server-streamable-http-core.rs)

**Start the server:**
_for instance, start the `hello-world-server-streamable-http`_
```sh
cargo run --example hello-world-server-streamable-http
```

Once the server starts, you’ll see the following output in the terminal:
```sh
• Streamable HTTP Server is available at http://127.0.0.1:8080/mcp
• SSE Server is available at http://127.0.0.1:8080/sse
```


For easy testing and inspection, connect to it using [MCP Inspector](https://github.com/modelcontextprotocol/inspector).
start the inspector by running:

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

<img src="../assets/examples/hello-world-server-streamable-http.gif" alt="hello-world-mcp-server-streamable-http" width="800" />

-----


### ➡️ MCP Server - Oauth Example

- [mcp-server-oauth-remote.rs](mcp-server-oauth-remote.rs)

A minimal, MCP server example that demonstrates **OAuth 2.0 / OpenID Connect authentication** using the  `RemoteAuthProvider` from [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk).

It features:
- Full OAuth 2.0 protection via bearer tokens
- Remote authentication metadata discovery
- Token verification using both JWKs and token introspection
- A single tool: `show_auth_info` - returns the authenticated user's claims and scopes in pretty-printed JSON

#### Overview

**RemoteAuthProvider** can be used with any OpenID Connect provider that supports Dynamic Client Registration (DCR), but in this example, it is configured to point to a local [Keycloak](https://www.keycloak.org) instance.

👉 For more information on how to start and configure your local Keycloak server, please refer to the  **keycloak-setup** section of the following blog post: https://modelcontextprotocol.io/docs/tutorials/security/authorization#keycloak-setup


#### Step 1:
Make sure you have a Keycloak server running and configured as described in this [blog post](https://modelcontextprotocol.io/docs/tutorials/security/authorization#keycloak-setup)

> 💡 _You can update the configuration in `create_oauth_provider()` function to connect to any other OAuth provider with DCR support or in case your keycloak configuration is different._

#### Step 2:
Set the `OAUTH_CLIENT_ID` and `OAUTH_CLIENT_SECRET` environment variables with the values from your keycloak server dashboard:

```
export OAUTH_CLIENT_ID=test-server OAUTH_CLIENT_SECRET=XYZ
```

#### Step 3:
start the server

```bash
cargo run --example mcp-server-oauth-remote
```

You will see:

```sh
• Streamable HTTP Server is available at http://[::1]:3000/
```

Now you can connect to it with [MCP Inspector](https://modelcontextprotocol.io/docs/tools/inspector), or alternatively, use it with any MCP client you prefer.

```bash
npx -y @modelcontextprotocol/inspector@latest
```

Here you can see it in action :

<img src="../assets/examples/mcp-remote-oauth.gif" alt="mcp-server-remote-oauth" width="800" />

-----


### ➡️ MCP Client Examples (stdio)
- [quick-start-client-stdio.rs](quick-start-client-stdio.rs)
- [simple-mcp-client-stdio.rs](simple-mcp-client-stdio.rs)
- [simple-mcp-client-stdio-core.rs](simple-mcp-client-stdio-core.rs)

These examples demonstrate an MCP client using the **stdio** transport, highlighting basic MCP client operations such as retrieving the MCP server's capabilities and making a tool call.

These examples launch the [@modelcontextprotocol/server-everything](https://www.npmjs.com/package/@modelcontextprotocol/server-everything) server, an MCP Server designed for experimenting with various capabilities of the MCP.

It prints the server name and version, outlines the server's capabilities, and provides a list of available tools, prompts, templates, resources, and more offered by the server. Additionally, it will execute a "tool call" , calling the `add` tool from the `server-everything` package to sum two numbers and output the result.

> Note that @modelcontextprotocol/server-everything is an npm package, so you must have Node.js and npm installed on your system, as this example attempts to start it.

**Start the server:**
_For instance, start the `simple-mcp-client-stdio`_
```sh
cargo run --example simple-mcp-client-stdio
```



Here you can observe a sample output of the project. however, your results may vary slightly depending on the version of the MCP Server in use when you run it.

<img src="../assets/examples/mcp-client-output.jpg" width="640"/>

-----


### ➡️ MCP Client Examples (Streamable HTTP)

- [simple-mcp-client-streamable-http.rs](simple-mcp-client-streamable-http.rs)
- [simple-mcp-client-streamable-http-core.rs](simple-mcp-client-streamable-http-core.rs)

These examples demonstrate an MCP client using the *Streamable HTTP* transport, highlighting basic MCP client operations such as retrieving the MCP server's capabilities and making a tool call.

These examples connect to a running instance of the [@modelcontextprotocol/server-everything](https://www.npmjs.com/package/@modelcontextprotocol/server-everything) server, which has already been started with the `streamableHttp` argument.

It displays the server name and version, outlines the server's capabilities, and provides a list of available tools, prompts, templates, resources, and more offered by the server. Additionally, it will execute a tool call by utilizing the add tool from the server-everything package to sum two numbers and output the result.


-----

### ➡️ MCP Client Examples (SSE)

- [simple-mcp-client-sse.rs](simple-mcp-client-sse.rs)
- [simple-mcp-client-sse-core.rs](simple-mcp-client-sse-core.rs)

These examples demonstrate an MCP client using the *SSE* transport, highlighting basic MCP client operations such as retrieving the MCP server's capabilities and making a tool call.

These examples connect to a running instance of the [@modelcontextprotocol/server-everything](https://www.npmjs.com/package/@modelcontextprotocol/server-everything) server, which has already been started with the `streamableHttp` argument.

It displays the server name and version, outlines the server's capabilities, and provides a list of available tools, prompts, templates, resources, and more offered by the server. Additionally, it will execute a tool call by utilizing the add tool from the server-everything package to sum two numbers and output the result.

1- First, start `@modelcontextprotocol/server-everything` with `sse` argument:
```bash
npx @modelcontextprotocol/server-everything sse
```
2- start the example client, for instance start the `simple-mcp-client-sse`:
```bash
cargo run --example simple-mcp-client-sse
```
