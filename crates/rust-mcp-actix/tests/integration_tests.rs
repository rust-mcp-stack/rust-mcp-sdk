use actix_web::{test, App};
use rust_mcp_actix::{mcp_scope, McpMountOptions};
use rust_mcp_sdk::id_generator::{FastIdGenerator, UuidGenerator};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler};
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{Implementation, InitializeResult, ProtocolVersion, ServerCapabilities};
use rust_mcp_sdk::session_store::InMemorySessionStore;
use rust_mcp_sdk::ToMcpServerHandler;
use std::sync::Arc;

fn test_server_details() -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: "test-server".into(),
            version: "0.1.0".into(),
            title: None,
            description: None,
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities::default(),
        meta: None,
        instructions: None,
        protocol_version: ProtocolVersion::V2025_11_25.into(),
    }
}

#[derive(Default)]
struct DummyHandler;
impl ServerHandler for DummyHandler {}

fn make_state() -> (Arc<McpAppState>, Arc<McpHttpHandler>) {
    let state = Arc::new(McpAppState {
        session_store: Arc::new(InMemorySessionStore::new()),
        id_generator: Arc::new(UuidGenerator {}),
        stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
        server_details: Arc::new(test_server_details()),
        handler: DummyHandler.to_mcp_server_handler(),
        ping_interval: std::time::Duration::from_secs(12),
        transport_options: Default::default(),
        enable_json_response: false,
        event_store: None,
        task_store: None,
        client_task_store: None,
        message_observer: None,
    });
    let handler = Arc::new(McpHttpHandler::new(None, vec![], None));
    (state, handler)
}

fn make_state_json_mode() -> (Arc<McpAppState>, Arc<McpHttpHandler>) {
    let (state, handler) = make_state();
    let state = McpAppState {
        enable_json_response: true,
        ..Arc::unwrap_or_clone(state)
    };
    (Arc::new(state), handler)
}

fn default_mount() -> McpMountOptions {
    McpMountOptions {
        streamable_http_endpoint: "/mcp".into(),
        sse_endpoint: "/sse".into(),
        sse_messages_endpoint: "/messages".into(),
        health_endpoint: Some("/health".into()),
        ..Default::default()
    }
}

// =====================================================================
// Basic route tests
// =====================================================================

#[actix_web::test]
async fn test_mcp_scope_routes_health() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status() != 404);
}

#[actix_web::test]
async fn test_health_endpoint_returns_200() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert!(body["server"].is_string());
    assert!(body["version"].is_string());
}

#[actix_web::test]
async fn test_health_endpoint_disabled_when_none() {
    let (state, handler) = make_state();
    let opts = McpMountOptions {
        health_endpoint: None,
        ..default_mount()
    };
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_unknown_path_returns_404() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::get()
        .uri("/non-existent-path")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_reject_oversized_request_body() {
    let (state, handler) = make_state();
    let opts = McpMountOptions {
        max_request_body_size: 1024,
        ..default_mount()
    };
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let oversized_body = "x".repeat(4096);
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_payload(oversized_body)
        .insert_header(("Content-Type", "application/json"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 413);
}

// =====================================================================
// Streamable HTTP route tests
// =====================================================================

#[actix_web::test]
async fn test_streamable_http_get_returns_error_without_session() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::get().uri("/mcp").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(!resp.status().is_success());
}

#[actix_web::test]
async fn test_streamable_http_post_invalid_body() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_payload("not-valid-json")
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Accept", "application/json, text/event-stream"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(!resp.status().is_success());
}

#[actix_web::test]
async fn test_streamable_http_post_non_utf8_body_rejected() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let non_utf8 = vec![0xFFu8, 0xFE, 0xFD];
    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_payload(non_utf8)
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Accept", "application/json, text/event-stream"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_streamable_http_delete_returns_error_without_session() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::delete().uri("/mcp").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(!resp.status().is_success());
}

// =====================================================================
// SSE streaming endpoint
// =====================================================================

#[actix_web::test]
async fn test_sse_endpoint_returns_event_stream() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::get()
        .uri("/sse")
        .insert_header(("Accept", "text/event-stream"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 200);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("text/event-stream"),
        "Expected text/event-stream, got: {}",
        content_type
    );
}

#[actix_web::test]
async fn test_streamable_http_init_returns_session_id() {
    let (state, handler) = make_state_json_mode();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::post()
        .uri("/mcp")
        .set_payload(serde_json::json!({
            "jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"1"}}
        }).to_string())
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Accept", "application/json, text/event-stream"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "Expected 2xx, got {}",
        resp.status()
    );
    let session_id = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .expect("Expected mcp-session-id header");
    assert!(!session_id.is_empty());
}

// =====================================================================
// Messages endpoint
// =====================================================================

#[actix_web::test]
async fn test_messages_endpoint_rejects_invalid_session() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::post()
        .uri("/messages")
        .set_payload("{}")
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Accept", "application/json, text/event-stream"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(!resp.status().is_success());
}

// =====================================================================
// Error status code mapping
// =====================================================================

#[actix_web::test]
async fn test_error_mapping_session_missing_returns_400() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::post()
        .uri("/messages")
        .set_payload("{}")
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Accept", "application/json, text/event-stream"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_error_mapping_invalid_session_returns_404() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::post()
        .uri("/messages?sessionId=nonexistent")
        .set_payload("{}")
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Accept", "application/json, text/event-stream"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}
