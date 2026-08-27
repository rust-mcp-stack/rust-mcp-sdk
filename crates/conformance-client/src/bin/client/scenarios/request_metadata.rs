//! `request-metadata` scenario (SEP-2575) — per-request `_meta` and
//! `MCP-Protocol-Version` header obligations on the stateless wire:
//!   - every POST carries the `MCP-Protocol-Version` header,
//!   - every request `_meta` carries `io.modelcontextprotocol/protocolVersion`,
//!     `io.modelcontextprotocol/clientCapabilities` and
//!     `io.modelcontextprotocol/clientInfo`,
//!   - the header version matches `_meta.protocolVersion`,
//!   - declared client capabilities (`roots`, `sampling`, `elicitation`) are
//!     well-formed objects,
//!   - when the server rejects the first-chosen version with
//!     `UNSUPPORTED_PROTOCOL_VERSION` (-32022), the client retries with one of
//!     the server's advertised `supported` versions.
//!
//! The scenario server rejects the first request with -32022 (advertising
//! only the current spec version) and then validates every subsequent
//! request. The SDK stamps the header at transport construction and `_meta`
//! on every request, so the retry just re-issues the call.

use rust_mcp_sdk::error::McpSdkError;
use rust_mcp_sdk::schema::schema_utils::RpcErrorCodes;
use rust_mcp_sdk::schema::RequestParams;
use rust_mcp_sdk::TransportError;

use crate::client::transport;

pub async fn run(server_url: &str) {
    let client = transport::connect(server_url)
        .await
        .expect("Failed to connect");

    // First request: the server simulates a version rejection (-32022).
    let first = client.request_discover(RequestParams::default()).await;
    match &first {
        Err(err) => {
            // The transport surfaces non-2xx JSON-RPC error bodies as
            // `TransportError::JsonrpcError`; a plain JSON-RPC error (2xx) would
            // arrive as `McpSdkError::RpcError`. Accept either shape.
            let code = match err {
                McpSdkError::RpcError(e) => Some(e.code),
                McpSdkError::Transport(TransportError::JsonrpcError(e)) => Some(e.code),
                _ => None,
            };
            assert_eq!(
                code,
                Some(RpcErrorCodes::UNSUPPORTED_PROTOCOL_VERSION as i64),
                "first request should be rejected with UNSUPPORTED_PROTOCOL_VERSION, got: {err:?}"
            );
        }
        Ok(result) => panic!("first request should fail with -32022, got: {result:?}"),
    }

    // Retry with a supported version (the runtime already advertises the
    // current spec version, which the server accepts).
    client
        .request_discover(RequestParams::default())
        .await
        .expect("retry with supported version should succeed");

    // Exercise a second request type so every per-request metadata check
    // (header, _meta, capabilities) is observed on multiple requests.
    client
        .request_tool_list(None)
        .await
        .expect("tools/list should succeed");

    client.shut_down().await.ok();
}
