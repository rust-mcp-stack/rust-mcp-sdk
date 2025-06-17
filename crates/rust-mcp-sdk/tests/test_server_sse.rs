#[path = "common/common.rs"]
pub mod common;
#[cfg(feature = "hyper-server")]
mod tets_server_sse {
    use std::{sync::Arc, time::Duration};

    use crate::common::{
        sse_data, sse_event,
        test_server_common::{
            collect_sse_lines, create_test_server, TestIdGenerator, INITIALIZE_REQUEST,
        },
    };
    use reqwest::Client;
    use rust_mcp_sdk::mcp_server::HyperServerOptions;
    use rust_mcp_sdk::schema::{
        schema_utils::{ResultFromServer, ServerMessage},
        ServerResult,
    };
    use tokio::time::sleep;

    #[tokio::test]
    async fn tets_sse_endpoint_event_default() {
        let server_options = HyperServerOptions {
            port: 8081,
            session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
                "AAA-BBB-CCC".to_string()
            ]))),
            ..Default::default()
        };

        let base_url = format!("http://{}:{}", server_options.host, server_options.port);

        let server_endpoint = format!("{}{}", base_url, server_options.sse_endpoint());

        let server = create_test_server(server_options);
        let handle = server.server_handle();
        let server_task = tokio::spawn(async move {
            server.start().await.unwrap();
            eprintln!("Server 1 is down");
        });

        sleep(Duration::from_millis(750)).await;

        let client = Client::new();
        println!("connecting to : {}", server_endpoint);
        // Act: Connect to the SSE endpoint and read the event stream
        let response = client
            .get(server_endpoint)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .expect("Failed to connect to SSE endpoint");

        assert_eq!(
            response.headers().get("content-type").map(|v| v.as_bytes()),
            Some(b"text/event-stream" as &[u8]),
            "Response content-type should be text/event-stream"
        );

        let lines = collect_sse_lines(response, 2, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(sse_event(&lines[0]), "endpoint");
        assert_eq!(sse_data(&lines[1]), "/messages?sessionId=AAA-BBB-CCC");

        let message_endpoint = format!("{}{}", base_url, sse_data(&lines[1]));
        let res = client
            .post(message_endpoint)
            .header("Content-Type", "application/json")
            .body(INITIALIZE_REQUEST.to_string())
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success());
        handle.graceful_shutdown(Some(Duration::from_millis(1)));
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn tets_sse_message_endpoint_query_hash() {
        let server_options = HyperServerOptions {
            port: 8082,
            custom_messages_endpoint: Some(
                "/custom-msg-endpoint?something=true&otherthing=false#section-59".to_string(),
            ),
            session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
                "AAA-BBB-CCC".to_string()
            ]))),
            ..Default::default()
        };

        let base_url = format!("http://{}:{}", server_options.host, server_options.port);

        let server_endpoint = format!("{}{}", base_url, server_options.sse_endpoint());

        let server = create_test_server(server_options);
        let handle = server.server_handle();

        let server_task = tokio::spawn(async move {
            server.start().await.unwrap();
            eprintln!("Server 2 is down");
        });

        sleep(Duration::from_millis(750)).await;

        let client = Client::new();
        println!("connecting to : {}", server_endpoint);
        // Act: Connect to the SSE endpoint and read the event stream
        let response = client
            .get(server_endpoint)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .expect("Failed to connect to SSE endpoint");

        assert_eq!(
            response.headers().get("content-type").map(|v| v.as_bytes()),
            Some(b"text/event-stream" as &[u8]),
            "Response content-type should be text/event-stream"
        );

        let lines = collect_sse_lines(response, 2, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(sse_event(&lines[0]), "endpoint");
        assert_eq!(
            sse_data(&lines[1]),
            "/custom-msg-endpoint?something=true&otherthing=false&sessionId=AAA-BBB-CCC#section-59"
        );

        let message_endpoint = format!("{}{}", base_url, sse_data(&lines[1]));
        let res = client
            .post(message_endpoint)
            .header("Content-Type", "application/json")
            .body(INITIALIZE_REQUEST.to_string())
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success());
        handle.graceful_shutdown(Some(Duration::from_millis(1)));
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn tets_sse_custom_message_endpoint() {
        let server_options = HyperServerOptions {
            port: 8083,
            custom_messages_endpoint: Some(
                "/custom-msg-endpoint?something=true&otherthing=false#section-59".to_string(),
            ),
            session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
                "AAA-BBB-CCC".to_string()
            ]))),
            ..Default::default()
        };

        let base_url = format!("http://{}:{}", server_options.host, server_options.port);

        let server_endpoint = format!("{}{}", base_url, server_options.sse_endpoint());

        let server = create_test_server(server_options);
        let handle = server.server_handle();

        let server_task = tokio::spawn(async move {
            server.start().await.unwrap();
            eprintln!("Server 3 is down");
        });

        sleep(Duration::from_millis(750)).await;

        let client = Client::new();
        println!("connecting to : {}", server_endpoint);
        // Act: Connect to the SSE endpoint and read the event stream
        let response = client
            .get(server_endpoint)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .expect("Failed to connect to SSE endpoint");

        assert_eq!(
            response.headers().get("content-type").map(|v| v.as_bytes()),
            Some(b"text/event-stream" as &[u8]),
            "Response content-type should be text/event-stream"
        );

        let message_endpoint = format!(
            "{}{}",
            base_url,
            "/custom-msg-endpoint?something=true&otherthing=false&sessionId=AAA-BBB-CCC#section-59"
        );
        let res = client
            .post(message_endpoint)
            .header("Content-Type", "application/json")
            .body(INITIALIZE_REQUEST.to_string())
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success());

        let lines = collect_sse_lines(response, 5, Duration::from_secs(5))
            .await
            .unwrap();

        let init_response = sse_data(&lines[3]);
        let result = serde_json::from_str::<ServerMessage>(&init_response).unwrap();

        assert!(matches!(result, ServerMessage::Response(response)
        if matches!(&response.result, ResultFromServer::ServerResult(server_result)
        if matches!(server_result, ServerResult::InitializeResult(init_result) if init_result.server_info.name == "Test MCP Server"))));
        handle.graceful_shutdown(Some(Duration::from_millis(1)));
        server_task.await.unwrap();
    }
}
