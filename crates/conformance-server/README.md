# conformance-server (MCP 2026-07-28)

A Rust MCP conformance test server built with [`rust-mcp-sdk`](https://github.com/rust-mcp-stack/rust-mcp-sdk). Implements all scenarios from the [MCP Conformance Test Suite](https://github.com/modelcontextprotocol/conformance) targeting the **2026-07-28 stateless spec**.

## Conformance Status

**110/110 scenarios passing** — all scenarios pass. No known limitations.

## Quick Start

```bash
cargo run -p conformance-server
```

Starts on `http://0.0.0.0:3101`. Configure the port with `MCP_CONFORMANCE_PORT` (default: `3101`).

## Endpoints

| Path | Method | Description |
|------|--------|-------------|
| `/mcp` | POST | Stateless MCP endpoint |
| `/health` | GET | Health check |

The 2026-07-28 protocol is stateless and POST-only. No GET, SSE, `/sse`, or `/messages` endpoints.

## Running Conformance Tests

```bash
cargo run -p conformance-server &
npx @modelcontextprotocol/conformance@0.2.0-alpha.10 server --url http://127.0.0.1:3101/mcp --spec-version 2026-07-28 --suite all
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
| `test_sampling` | Requests LLM sampling from client (MRTR) |
| `test_elicitation` | Requests user input from client (MRTR) |
| `test_elicitation_sep1034_defaults` | Elicitation schema with defaults for all primitive types |
| `test_elicitation_sep1330_enums` | Elicitation schema with all 5 enum variants |
| `test_elicitation_with_input_required` | Elicitation via InputRequiredResult (MRTR) |

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

### Protocol Features

- **Stateless transport** — POST-only, no sessions, no SSE streaming
- **InputRequiredResult (MRTR)** — tools request elicitation/sampling inline
- **Response cache (SEP-2549)** — principal-scoped caching with `ttlMs` and `cacheScope`
- **Logging** — all severity levels (debug through emergency)
- **Completions** — prompt argument autocompletion
- **Resource subscriptions** — subscribe/unsubscribe with update notifications
- **DNS rebinding protection** — Host header validation for localhost servers
- **RequestContext** — per-request `_meta` with authentication context

## Known Limitations

None — all 110 conformance scenarios pass.

## Tech Stack

- **Rust** 1.80+
- [`rust-mcp-sdk`](https://crates.io/crates/rust-mcp-sdk) — MCP protocol implementation
- [`rust-mcp-axum`](https://crates.io/crates/rust-mcp-axum) — Axum HTTP integration
- Stateless HTTP transport (POST-only)
- Protocol version: 2026-07-28 (stateless)
