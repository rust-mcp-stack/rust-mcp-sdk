import React from 'react';
import { useHistory } from '@docusaurus/router';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import CodeBlock from '@theme/CodeBlock';

const features = [
  {
    title: '100% Conformance',
    emoji: '🎯',
    description: 'Passes all server and client MCP conformance tests - protocol version 2025-11-25.',
  },
  {
    title: 'All Transports',
    emoji: '🔌',
    description: 'Stdio, Streamable HTTP, and backward-compatible SSE. One handler works across all transports.',
  },
  {
    title: 'Framework Agnostic',
    emoji: '🧩',
    description: 'Native Axum and Actix-web integrations, plus BYO-server mode for existing apps.',
  },
  {
    title: 'Production Ready',
    emoji: '🏭',
    description: 'DNS rebinding protection, resumability, health checks, OAuth 2.1, and MCP Tasks.',
  },
  {
    title: 'Powerful Macros',
    emoji: '✨',
    description: 'mcp_tool, tool_box!, mcp_elicit - generate MCP schemas directly from your Rust structs.',
  },
  {
    title: 'Open & Free',
    emoji: '🦀',
    description: 'MIT licensed. Part of the rust-mcp-stack ecosystem with rust-mcp-filesystem and mcp-discovery.',
  },
];

function Feature({ emoji, title, description }: { emoji: string; title: string; description: string }) {
  return (
    <Link className="col col--4 margin-bottom--lg" to="/docs/getting-started/welcome">
      <div className="card" style={{ height: '100%' }}>
        <div className="card__body">
          <div style={{ fontSize: '1.75rem', marginBottom: '0.75rem' }}>{emoji}</div>
          <h3 style={{ fontSize: '1rem', fontWeight: 600, marginBottom: '0.5rem' }}>{title}</h3>
          <p style={{ fontSize: '0.8125rem', color: 'var(--content-secondary-color)', lineHeight: 1.6 }}>{description}</p>
        </div>
      </div>
    </Link>
  );
}

export default function Home() {
  return (
    <Layout title="Rust MCP SDK" description="A high-performance, asynchronous Rust toolkit for building MCP servers and clients.">
      {/* Hero */}
      <header className="hero" style={{ }}>
        <div className="container">
          <img src='img/rust-mcp-sdk.png' width={148} style={{borderRadius:"0.75rem"}}/>
          <h1 className="hero__title" style={{ fontSize: 'clamp(2rem, 5vw, 3.5rem)' }}>
            Rust MCP SDK
          </h1>
          <p className="hero__subtitle" style={{ fontSize: '1.125rem', maxWidth: '640px' }}>
            A high-performance, asynchronous Rust toolkit for building MCP servers and clients.
            Focus on your application logic - we handle the protocol, transports, and the rest.
          </p>
          <div className="hero__buttons">
            <Link className="button button--primary" to="/docs/getting-started/quickstart">
              Get Started →
            </Link>
            <Link className="button button--secondary" to="/docs/getting-started/welcome">
              Learn More
            </Link>
          </div>
          <div style={{ marginTop: '2rem', display: 'flex', gap: '8px', justifyContent: 'center', flexWrap: 'wrap' }}>
            <img alt="crates.io" src="https://img.shields.io/crates/v/rust-mcp-sdk?style=flat-square&logo=rust&color=FE965D" height="20" />
            <img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-rust_mcp_SDK-0ECDAB?style=flat-square&logo=docs.rs" height="20" />
            <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/rust-mcp-stack/rust-mcp-sdk/ci.yml?style=flat-square" height="20" />
          </div>
        </div>
      </header>

      {/* Features */}
      <main style={{ padding: '64px 0' }}>
        <div className="container">
          <h2 style={{ textAlign: 'center', fontSize: '1.5rem', fontWeight: 700, marginBottom: '3rem' }}>
            Everything you need for MCP in Rust
          </h2>
          <div className="row">
            {features.map((props, idx) => (
              <Feature key={idx} {...props} />
            ))}
          </div>
        </div>
      </main>

      {/* Quickstart snippet */}
      <div style={{ background: 'var(--ifm-background-surface-color)', padding: '64px 0' }}>
        <div className="container" style={{ maxWidth: '720px' }}>
          <h2 style={{ textAlign: 'center', fontSize: '1.5rem', fontWeight: 700, marginBottom: '1.5rem' }}>
            Build an MCP server in 3 steps
          </h2>
          <CodeBlock language="rust">
{`use async_trait::async_trait;
use rust_mcp_sdk::*;
use std::sync::Arc;

// Step 1 - Define a tool
#[macros::mcp_tool(name = "say_hello", description = "Returns a friendly greeting")]
#[derive(Debug, serde::Deserialize, serde::Serialize, macros::JsonSchema)]
pub struct SayHelloTool { pub name: String }

// Step 2 - Implement the handler
#[async_trait]
impl ServerHandler for MyServer {
    async fn handle_list_tools_request(
        &self, _: Option<PaginatedRequestParams>, _: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult { tools: vec![SayHelloTool::tool()], ..Default::default() })
    }

    async fn handle_call_tool_request(
        &self, params: CallToolRequestParams, _: Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        let input: SayHelloTool = params.arguments.parse()?;
        Ok(CallToolResult::text_content(vec![format!("Hello, {}!", input.name)]))
    }
}

// Step 3 - Start the server
#[tokio::main]
async fn main() -> SdkResult<()> {
    let server = server_runtime::create_server(McpServerOptions {
        transport: StdioTransport::new(TransportOptions::default()?)?,
        handler: MyServer.to_mcp_server_handler(),
        ..Default::default()
    });
    server.start().await
}`}
          </CodeBlock>
          <div style={{ textAlign: 'center', marginTop: '2rem' }}>
            <Link className="button button--primary" to="/docs/getting-started/quickstart">
              Try the Quickstart →
            </Link>
          </div>
        </div>
      </div>

    </Layout>
  );
}
