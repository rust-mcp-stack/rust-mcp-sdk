use rust_mcp_actix::ActixMountOptions;

#[test]
fn test_actix_mount_options_default() {
    let mount = ActixMountOptions::default();
    assert_eq!(mount.streamable_http_endpoint, "/mcp");
    assert_eq!(mount.sse_endpoint, "/sse");
    assert_eq!(mount.sse_messages_endpoint, "/messages");
    assert!(mount.health_endpoint.is_none());
}

#[test]
fn test_actix_mount_options_custom() {
    let mount = ActixMountOptions {
        streamable_http_endpoint: "/api/mcp".into(),
        sse_endpoint: "/api/sse".into(),
        sse_messages_endpoint: "/api/messages".into(),
        health_endpoint: Some("/api/health".into()),
    };
    assert_eq!(mount.streamable_http_endpoint, "/api/mcp");
    assert_eq!(mount.sse_endpoint, "/api/sse");
    assert_eq!(mount.sse_messages_endpoint, "/api/messages");
    assert_eq!(mount.health_endpoint, Some("/api/health".into()));
}
