#[path = "common/common.rs"]
pub mod common;

use std::time::Duration;

use rust_mcp_sdk::mcp_client::{client_runtime, McpClientOptions};
use rust_mcp_sdk::schema::{PaginatedRequestParams, RequestMetaObject, RequestParams};
use rust_mcp_sdk::{McpClient, StdioTransport, ToMcpClientHandler, TransportOptions};
use tokio::time::timeout;

use common::{test_client_info, TestClientHandler};

const SOAK_CONCURRENT_CLIENTS: usize = 8;
const SOAK_REQUESTS_PER_CLIENT: usize = 20;

fn default_request_meta() -> RequestMetaObject {
    RequestMetaObject {
        client_capabilities: Default::default(),
        client_info: None,
        log_level: None,
        protocol_version: "2026-07-28".to_string(),
        progress_token: None,
        extra: None,
    }
}

/// Soak test: concurrent discover + tool_list requests against a real conformance server.
/// Requires the conformance-server binary (cargo build -p conformance-server).
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "requires conformance-server binary; run: cargo build -p conformance-server"]
async fn soak_concurrent_discover_and_list() {
    let transport = StdioTransport::create_with_server_launch(
        "./target/debug/conformance-server",
        vec![],
        None,
        TransportOptions::default(),
    )
    .expect("failed to launch conformance-server");

    let client = client_runtime::create_client(McpClientOptions::new(
        test_client_info(),
        transport,
        TestClientHandler {}.to_mcp_client_handler(),
    ));

    client
        .clone()
        .start()
        .await
        .expect("client failed to start");

    let mut handles = Vec::new();
    for _ in 0..SOAK_CONCURRENT_CLIENTS {
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..SOAK_REQUESTS_PER_CLIENT {
                let _ = timeout(Duration::from_secs(10), async {
                    let discover = c
                        .request_discover(RequestParams {
                            meta: default_request_meta(),
                        })
                        .await;
                    assert!(discover.is_ok(), "discover failed at iteration {i}");

                    let tools = c
                        .request_tool_list(Some(PaginatedRequestParams {
                            cursor: None,
                            meta: default_request_meta(),
                        }))
                        .await;
                    assert!(tools.is_ok(), "tool list failed at iteration {i}");
                })
                .await
                .expect("request timed out");
            }
        }));
    }

    for h in handles {
        h.await.expect("soak task panicked");
    }

    let _ = client.shut_down().await;
}

/// Soak test: concurrent JSON-RPC request/response serialization roundtrips.
#[tokio::test(flavor = "multi_thread")]
async fn soak_mcp_request_serialization_roundtrip() {
    use rust_mcp_sdk::schema::schema_utils::{ClientJsonrpcRequest, ClientJsonrpcResponse};
    use rust_mcp_sdk::schema::{ListRootsResult, RequestId};

    let result = ListRootsResult { roots: vec![] };

    let concurrent: i64 = 8;
    let per_client: i64 = 50;

    let mut handles = Vec::new();
    for t in 0..concurrent {
        let result = result.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..per_client {
                let id = t * per_client + i;

                let request = ClientJsonrpcRequest::new(
                    RequestId::Integer(id),
                    rust_mcp_sdk::schema::schema_utils::RequestFromClient::DiscoverRequest(
                        Default::default(),
                    ),
                );

                let serialized = serde_json::to_string(&request).expect("serialize request");
                let _deserialized: ClientJsonrpcRequest =
                    serde_json::from_str(&serialized).expect("deserialize request");

                let response =
                    ClientJsonrpcResponse::new(RequestId::Integer(id), result.clone().into());

                let serialized = serde_json::to_string(&response).expect("serialize response");
                let _deserialized: ClientJsonrpcResponse =
                    serde_json::from_str(&serialized).expect("deserialize response");
            }
        }));
    }

    for h in handles {
        h.await.expect("soak task panicked");
    }
}
