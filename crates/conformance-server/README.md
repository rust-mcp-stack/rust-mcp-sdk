# conformance-server

A Rust MCP conformance test server built with [`rust-mcp-sdk`](https://github.com/rust-mcp-stack/rust-mcp-sdk). Implements all scenarios from the [MCP Conformance Test Suite](https://github.com/modelcontextprotocol/conformance) targeting the **2025-11-25 stateful spec**.

## Conformance Status

**39/40 scenarios passing** — 1 known limitation (`tools-call-with-progress`, test-framework level).

## Quick Start

```bash
cargo run -p conformance-server
```

Starts on `http://0.0.0.0:3000`.

## Endpoints

| Path | Method | Description |
|------|--------|-------------|
| `/mcp` | POST | Streamable HTTP MCP endpoint |
| `/mcp` | GET | SSE notification stream |
| `/sse` | GET | SSE endpoint (backward compat) |
| `/messages` | POST | SSE messages endpoint (backward compat) |
| `/health` | GET | Health check |

## Running Conformance Tests

```bash
cargo run -p conformance-server &
npx @modelcontextprotocol/conformance server --url http://localhost:3000/mcp
```

## Implemented Features

### Tools (13)

| Name | Description |
|------|-------------|
| `test_simple_text` | Returns simple text content |
| `test_image_content` | Returns base64 PNG image |
| `test_audio_content` | Returns base64 WAV audio |
| `test_embedded_resource` | Returns embedded resource content |
| `test_multiple_content_types` | Returns text + image + resource |
| `test_error_handling` | Returns `isError: true` |
| `test_tool_with_logging` | Emits 3 log notifications during execution |
| `test_tool_with_progress` | Reports 3 progress notifications (0/50/100) |
| `test_sampling` | Requests LLM sampling from client |
| `test_elicitation` | Requests user input from client |
| `test_elicitation_sep1034_defaults` | Elicitation schema with defaults for all primitive types |
| `test_elicitation_sep1330_enums` | Elicitation schema with all 5 enum variants |

### Resources (5)

| URI | MIME | Description |
|-----|------|-------------|
| `test://static-text` | text/plain | Static text content |
| `test://static-binary` | image/png | Static binary (base64 PNG) |
| `test://template/{id}/data` | application/json | Template with parameter substitution |
| `test://embedded-resource` | text/plain | Resource for embedded content tests |
| `test://watched-resource` | application/json | Subscribable resource |

### Prompts (4)

| Name | Arguments | Description |
|------|-----------|-------------|
| `test_simple_prompt` | — | Simple text prompt |
| `test_prompt_with_arguments` | `arg1`, `arg2` | Parameterized prompt |
| `test_prompt_with_embedded_resource` | `resourceUri` | Prompt with embedded resource |
| `test_prompt_with_image` | — | Prompt with image content |

### Other Capabilities

- **Logging** — all severity levels (debug through emergency)
- **Completions** — prompt argument autocompletion
- **Resource subscriptions** — subscribe/unsubscribe with update notifications
- **DNS rebinding protection** — Host header validation for localhost servers
- **Ping** — connection health check

## Known Limitations

| Scenario | Status | Reason |
|----------|--------|--------|
| `tools-call-with-progress` | Expected failure | Test-framework Zod schema identity mismatch in `connectStateful()` — notifications reach the transport but aren't dispatched to the test's handler array |

## Tech Stack

- **Rust** 1.80+
- [`rust-mcp-sdk`](https://crates.io/crates/rust-mcp-sdk) — MCP protocol implementation
- [`rust-mcp-axum`](https://crates.io/crates/rust-mcp-axum) — Axum HTTP integration
- Streamable HTTP transport (SSE + JSON responses)
- Protocol version: 2025-11-25 (stateful)
