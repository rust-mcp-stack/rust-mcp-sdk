use rust_mcp_sdk::mcp_http::McpMountOptions;

#[test]
fn test_mount_options_default() {
    let mount = McpMountOptions::default();
    assert_eq!(mount.streamable_http_endpoint, "/mcp");
    assert_eq!(mount.sse_endpoint, "/sse");
    assert_eq!(mount.sse_messages_endpoint, "/messages");
    assert!(mount.health_endpoint.is_none());
    assert_eq!(mount.max_request_body_size, 4 * 1024 * 1024);
}

#[test]
fn test_mount_options_custom() {
    let mount = McpMountOptions {
        streamable_http_endpoint: "/api/mcp".into(),
        sse_endpoint: "/api/sse".into(),
        sse_messages_endpoint: "/api/messages".into(),
        health_endpoint: Some("/api/health".into()),
        max_request_body_size: 2048,
    };
    assert_eq!(mount.streamable_http_endpoint, "/api/mcp");
    assert_eq!(mount.sse_endpoint, "/api/sse");
    assert_eq!(mount.sse_messages_endpoint, "/api/messages");
    assert_eq!(mount.health_endpoint, Some("/api/health".into()));
    assert_eq!(mount.max_request_body_size, 2048);
}

#[test]
fn test_create_actix_server_returns_server() {
    use rust_mcp_sdk::mcp_server::ServerHandler;
    use rust_mcp_sdk::schema::{
        Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
    };
    use rust_mcp_sdk::ToMcpServerHandler;

    #[derive(Default)]
    struct DummyHandler;
    impl ServerHandler for DummyHandler {}

    let details = InitializeResult {
        server_info: Implementation {
            name: "test".into(),
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
    };

    let handler = DummyHandler::default().to_mcp_server_handler();
    let options = rust_mcp_actix::ActixServerOptions::default();

    let server = rust_mcp_actix::create_actix_server(details, handler, options);
    assert!(server.server_info(None).is_ok());
}

#[test]
fn test_actix_server_options_default_valid() {
    let options = rust_mcp_actix::ActixServerOptions::default();
    assert!(options.validate().is_ok());
}

#[test]
fn test_actix_server_options_resolve_address() {
    let options = rust_mcp_actix::ActixServerOptions::default();
    let addr = options.resolve_server_address();
    assert!(addr.is_ok());
}

#[test]
fn test_actix_server_options_custom_endpoints() {
    let options = rust_mcp_actix::ActixServerOptions {
        custom_streamable_http_endpoint: Some("/v2/mcp".into()),
        custom_sse_endpoint: Some("/v2/sse".into()),
        custom_messages_endpoint: Some("/v2/msg".into()),
        ..Default::default()
    };
    assert_eq!(options.streamable_http_endpoint(), "/v2/mcp");
    assert_eq!(options.sse_endpoint(), "/v2/sse");
    assert_eq!(options.sse_messages_endpoint(), "/v2/msg");
}

#[test]
fn test_resolve_mount_options_default() {
    let options = rust_mcp_actix::ActixServerOptions::default();
    let mount = options.resolve_mount_options();
    assert_eq!(mount.streamable_http_endpoint, "/mcp");
    assert_eq!(mount.sse_endpoint, "/sse");
    assert_eq!(mount.sse_messages_endpoint, "/messages");
    assert!(mount.health_endpoint.is_none());
    assert_eq!(mount.max_request_body_size, 4 * 1024 * 1024);
}

#[test]
fn test_resolve_mount_options_custom_body_limit() {
    let options = rust_mcp_actix::ActixServerOptions {
        custom_streamable_http_endpoint: Some("/v2/mcp".into()),
        max_request_body_size: Some(4096),
        ..Default::default()
    };
    let mount = options.resolve_mount_options();
    assert_eq!(mount.max_request_body_size, 4096);
}
