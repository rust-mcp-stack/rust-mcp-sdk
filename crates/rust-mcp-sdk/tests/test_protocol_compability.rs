#[path = "common/common.rs"]
pub mod common;

mod protocol_compability_on_server {

    use rust_mcp_schema::{InitializeRequest, InitializeResult, RpcError, INTERNAL_ERROR};
    use rust_mcp_sdk::mcp_server::ServerHandler;

    use crate::common::{
        test_client_info,
        test_server_common::{test_server_details, TestServerHandler},
    };

    async fn handle_initialize_request(
        client_protocol_version: &str,
    ) -> Result<InitializeResult, RpcError> {
        let handler = TestServerHandler {};

        let mut initialize_request = test_client_info();
        initialize_request.protocol_version = client_protocol_version.to_string();

        let transport =
            rust_mcp_sdk::StdioTransport::new(rust_mcp_sdk::TransportOptions::default()).unwrap();

        // mock unused runtime
        let runtime = rust_mcp_sdk::mcp_server::server_runtime::create_server(
            test_server_details(),
            transport,
            TestServerHandler {},
        );

        handler
            .handle_initialize_request(InitializeRequest::new(initialize_request), &runtime)
            .await
    }

    #[tokio::test]
    async fn tets_protocol_compability_equal() {
        let result = handle_initialize_request("2025-03-26").await;
        assert!(result.is_ok());
        let protocol_version = result.unwrap().protocol_version;
        assert_eq!(protocol_version, "2025-03-26");
    }

    #[tokio::test]
    async fn tets_protocol_compability_downgrade() {
        let result = handle_initialize_request("2024_11_05").await;
        assert!(result.is_ok());
        let protocol_version = result.unwrap().protocol_version;
        assert_eq!(protocol_version, "2024_11_05");
    }

    #[tokio::test]
    async fn tets_protocol_compability_unsupported() {
        let result = handle_initialize_request("2034_11_05").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(err) if err.code == INTERNAL_ERROR));
    }
}
