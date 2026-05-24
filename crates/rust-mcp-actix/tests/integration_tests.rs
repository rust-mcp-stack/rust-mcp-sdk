use actix_web::{test, App};
use rust_mcp_actix::{mcp_scope, ActixMountOptions};
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
        handler: DummyHandler::default().to_mcp_server_handler(),
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

fn default_mount() -> ActixMountOptions {
    ActixMountOptions {
        streamable_http_endpoint: "/mcp".into(),
        sse_endpoint: "/sse".into(),
        sse_messages_endpoint: "/messages".into(),
        health_endpoint: Some("/health".into()),
    }
}

// =====================================================================
// Integration tests (actix runtime required)
// =====================================================================

#[actix_web::test]
async fn test_mcp_scope_routes_health() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    // Health endpoint should be registered when health_endpoint is Some
    assert!(
        resp.status().is_success() || resp.status() != 404,
        "Health route should not 404, got {}",
        resp.status()
    );
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
    let opts = ActixMountOptions {
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
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(!resp.status().is_success());
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

#[actix_web::test]
async fn test_sse_endpoint_registered() {
    // Verify that mcp_scope can be constructed and app initialized without panicking.
    // SSE connections are long-lived streams not suited for the synchronous test client.
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let _app = test::init_service(App::new().service(scope)).await;
}

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
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(!resp.status().is_success());
}

#[actix_web::test]
async fn test_error_bridge_session_id_missing() {
    let (state, handler) = make_state();
    let opts = default_mount();
    let scope = mcp_scope(state, handler, &opts);
    let app = test::init_service(App::new().service(scope)).await;

    let req = test::TestRequest::post()
        .uri("/messages")
        .set_payload("{}")
        .insert_header(("Content-Type", "application/json"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 500);
}
