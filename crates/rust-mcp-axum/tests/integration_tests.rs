use axum::body::Body;
use axum::http::{Method, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use rust_mcp_axum::{mcp_routes, AxumServerOptions, McpMountOptions};
use rust_mcp_sdk::id_generator::{FastIdGenerator, UuidGenerator};
use rust_mcp_sdk::mcp_http::McpAppState;
use rust_mcp_sdk::mcp_http::McpHttpHandler;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{Implementation, InitializeResult, ProtocolVersion, ServerCapabilities};
use rust_mcp_sdk::session_store::InMemorySessionStore;
use rust_mcp_sdk::ToMcpServerHandler;
use std::sync::Arc;
use tower::ServiceExt;

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

fn make_app(http_handler: McpHttpHandler, mount: &McpMountOptions) -> Router {
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
    mcp_routes(state, mount, http_handler)
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
// McpMountOptions tests
// =====================================================================

#[test]
fn test_axum_mount_options_default() {
    let mount = McpMountOptions::default();
    assert_eq!(mount.streamable_http_endpoint, "/mcp");
    assert_eq!(mount.sse_endpoint, "/sse");
    assert_eq!(mount.sse_messages_endpoint, "/messages");
    assert!(mount.health_endpoint.is_none());
    assert_eq!(mount.max_request_body_size, 4 * 1024 * 1024);
}

#[test]
fn test_axum_mount_options_custom() {
    let mount = McpMountOptions {
        streamable_http_endpoint: "/api/mcp".into(),
        sse_endpoint: "/api/sse".into(),
        sse_messages_endpoint: "/api/messages".into(),
        health_endpoint: Some("/api/health".into()),
        ..Default::default()
    };
    assert_eq!(mount.streamable_http_endpoint, "/api/mcp");
    assert_eq!(mount.sse_endpoint, "/api/sse");
    assert_eq!(mount.sse_messages_endpoint, "/api/messages");
    assert_eq!(mount.health_endpoint, Some("/api/health".into()));
}

// =====================================================================
// AxumServerOptions -> McpMountOptions bridge
// =====================================================================

#[test]
fn test_resolve_mount_options_default() {
    let options = AxumServerOptions::default();
    let mount = options.resolve_mount_options();
    assert_eq!(mount.streamable_http_endpoint, "/mcp");
    assert_eq!(mount.sse_endpoint, "/sse");
    assert_eq!(mount.sse_messages_endpoint, "/messages");
    assert!(mount.health_endpoint.is_none());
    assert_eq!(mount.max_request_body_size, 4 * 1024 * 1024);
}

#[test]
fn test_resolve_mount_options_custom() {
    let options = AxumServerOptions {
        custom_streamable_http_endpoint: Some("/custom/mcp".into()),
        custom_sse_endpoint: Some("/custom/sse".into()),
        custom_messages_endpoint: Some("/custom/msg".into()),
        health_endpoint: Some("/custom/health".into()),
        max_request_body_size: Some(1024),
        ..AxumServerOptions::default()
    };
    let mount = options.resolve_mount_options();
    assert_eq!(mount.streamable_http_endpoint, "/custom/mcp");
    assert_eq!(mount.sse_endpoint, "/custom/sse");
    assert_eq!(mount.sse_messages_endpoint, "/custom/msg");
    assert_eq!(mount.health_endpoint, Some("/custom/health".into()));
    assert_eq!(mount.max_request_body_size, 1024);
}

// =====================================================================
// Health check route tests
// =====================================================================

#[tokio::test]
async fn test_health_endpoint_returns_200() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(json["server"].is_string());
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn test_health_endpoint_disabled_when_none() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = McpMountOptions {
        health_endpoint: None,
        ..default_mount()
    };
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Fallback should handle it (404)
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================================
// Streamable HTTP route tests
// =====================================================================

#[tokio::test]
async fn test_streamable_http_get_without_session_id() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::GET)
                .uri("/mcp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Without sessionId query param, should get an error
    assert!(!response.status().is_success());
}

#[tokio::test]
async fn test_streamable_http_post_invalid_body() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("Content-Type", "application/json")
                .body(Body::from("not-valid-json"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should get an error response
    assert!(!response.status().is_success());
}

#[tokio::test]
async fn test_streamable_http_delete_without_session_id() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::DELETE)
                .uri("/mcp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Without sessionId query param, should get an error
    assert!(!response.status().is_success());
}

#[tokio::test]
async fn test_reject_oversized_request_body_byo() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = McpMountOptions {
        max_request_body_size: 1024,
        ..default_mount()
    };
    let app = make_app(handler, &mount);

    let oversized_body = "x".repeat(4096);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/mcp")
                .header("Content-Type", "application/json")
                .body(Body::from(oversized_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// =====================================================================
// Fallback route tests
// =====================================================================

#[tokio::test]
async fn test_fallback_unknown_path_returns_404() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::GET)
                .uri("/non-existent-path")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("does not exist"));
}

// =====================================================================
// SSE endpoint presence test
// =====================================================================

#[tokio::test]
async fn test_sse_endpoint_accepts_connection() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::GET)
                .uri("/sse")
                .header("Accept", "text/event-stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // SSE endpoint should accept the connection
    assert!(response.status().is_success());
}

// =====================================================================
// Messages endpoint test
// =====================================================================

#[tokio::test]
async fn test_messages_endpoint_rejects_invalid_session() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/messages")
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Without valid sessionId, should return an error
    assert!(!response.status().is_success());
}

// =====================================================================
// Error bridge: McpHttpError -> TransportServerError -> IntoResponse
// =====================================================================

#[tokio::test]
async fn test_error_bridge_session_id_missing() {
    let handler = McpHttpHandler::new(None, vec![], None);
    let mount = default_mount();
    let app = make_app(handler, &mount);

    // POST to /messages without sessionId query param
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/messages")
                .header("Content-Type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    // TransportServerError::SessionIdMissing maps to 500 via IntoResponse
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// =====================================================================
// create_axum_server factory
// =====================================================================

#[tokio::test]
async fn test_create_axum_server_factory() {
    let server_details = test_server_details();
    let handler = DummyHandler::default().to_mcp_server_handler();
    let options = AxumServerOptions::default();

    let server = rust_mcp_axum::create_axum_server(server_details, handler, options);

    let info = server.server_info(None).await.unwrap();
    assert!(info.contains("Streamable HTTP"));
}

// =====================================================================
// AxumServerOptions validation
// =====================================================================

#[test]
fn test_axum_server_options_default_valid() {
    let options = AxumServerOptions::default();
    assert!(options.validate().is_ok());
}

#[test]
fn test_axum_server_options_ssl_without_certs_fails() {
    let options = AxumServerOptions {
        enable_ssl: true,
        ..AxumServerOptions::default()
    };
    assert!(options.validate().is_err());
}

#[test]
fn test_axum_server_options_custom_endpoints_resolve() {
    let options = AxumServerOptions {
        custom_streamable_http_endpoint: Some("/v2/mcp".into()),
        custom_sse_endpoint: Some("/v2/sse".into()),
        custom_messages_endpoint: Some("/v2/msg".into()),
        ..AxumServerOptions::default()
    };
    assert_eq!(options.streamable_http_endpoint(), "/v2/mcp");
    assert_eq!(options.sse_endpoint(), "/v2/sse");
    assert_eq!(options.sse_messages_endpoint(), "/v2/msg");
}

#[test]
fn test_axum_server_options_base_url() {
    let options = AxumServerOptions {
        host: "0.0.0.0".into(),
        port: 9090,
        ..AxumServerOptions::default()
    };
    assert_eq!(options.base_url(), "http://0.0.0.0:9090");
}

#[test]
fn test_axum_server_options_base_url_ssl() {
    let options = AxumServerOptions {
        host: "api.example.com".into(),
        port: 443,
        enable_ssl: true,
        ..AxumServerOptions::default()
    };
    assert_eq!(options.base_url(), "https://api.example.com:443");
}

#[test]
fn test_axum_server_options_needs_dns_protection_disabled_by_default() {
    let options = AxumServerOptions::default();
    assert!(!options.needs_dns_protection());
}

#[test]
fn test_axum_server_options_needs_dns_protection_enabled_with_hosts() {
    let options = AxumServerOptions {
        dns_rebinding_protection: true,
        allowed_hosts: Some(vec!["127.0.0.1".into()]),
        ..AxumServerOptions::default()
    };
    assert!(options.needs_dns_protection());
}

#[test]
fn test_axum_server_options_needs_dns_protection_enabled_with_origins() {
    let options = AxumServerOptions {
        dns_rebinding_protection: true,
        allowed_origins: Some(vec!["http://localhost".into()]),
        ..AxumServerOptions::default()
    };
    assert!(options.needs_dns_protection());
}

#[test]
fn test_axum_server_options_dns_protection_requires_hosts_or_origins() {
    let options = AxumServerOptions {
        dns_rebinding_protection: true,
        ..AxumServerOptions::default()
    };
    assert!(!options.needs_dns_protection());
}

#[test]
fn test_axum_server_options_sse_url_methods() {
    let options = AxumServerOptions {
        host: "127.0.0.1".into(),
        port: 8080,
        custom_sse_endpoint: Some("/my-sse".into()),
        custom_messages_endpoint: Some("/my-msgs".into()),
        ..AxumServerOptions::default()
    };
    assert_eq!(options.sse_url(), "http://127.0.0.1:8080/my-sse");
    assert_eq!(options.sse_message_url(), "http://127.0.0.1:8080/my-msgs");
}
