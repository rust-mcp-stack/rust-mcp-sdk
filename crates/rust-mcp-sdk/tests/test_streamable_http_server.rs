use std::{collections::HashMap, error::Error, sync::Arc, time::Duration, vec};

use hyper::StatusCode;
use rust_mcp_schema::{
    schema_utils::{
        ClientJsonrpcRequest, ClientJsonrpcResponse, ClientMessage, ClientMessages, FromMessage,
        NotificationFromServer, RequestFromServer, ResultFromServer, RpcMessage, SdkError,
        SdkErrorCodes, ServerJsonrpcNotification, ServerJsonrpcRequest, ServerJsonrpcResponse,
        ServerMessages,
    },
    CallToolRequest, CallToolRequestParams, ListRootsResult, ListToolsRequest, LoggingLevel,
    LoggingMessageNotificationParams, RequestId, RootsListChangedNotification, ServerNotification,
    ServerRequest, ServerResult,
};
use rust_mcp_sdk::{event_store::InMemoryEventStore, mcp_server::HyperServerOptions};
use serde_json::{json, Map, Value};

use crate::common::{
    random_port, read_sse_event, read_sse_event_from_stream, send_delete_request, send_get_request,
    send_post_request,
    test_server_common::{
        create_start_server, initialize_request, LaunchedServer, TestIdGenerator,
    },
};

#[path = "common/common.rs"]
pub mod common;

const ONE_MILLISECOND: Option<Duration> = Some(Duration::from_millis(1));

async fn initialize_server(
    enable_json_response: Option<bool>,
) -> Result<(LaunchedServer, String), Box<dyn Error>> {
    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(0), initialize_request().into());

    let server_options = HyperServerOptions {
        port: random_port(),
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        enable_json_response,
        ping_interval: Duration::from_secs(1),
        event_store: Some(Arc::new(InMemoryEventStore::default())),
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    tokio::time::sleep(Duration::from_millis(250)).await;
    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None,
        None,
    )
    .await
    .expect("Request failed");

    let session_id = response
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    Ok((server, session_id))
}

// should initialize server and generate session ID
#[tokio::test]
async fn should_initialize_server_and_generate_session_id() {
    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(0), initialize_request().into());

    let server_options = HyperServerOptions {
        port: random_port(),
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    tokio::time::sleep(Duration::from_millis(250)).await;
    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None,
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert!(response.headers().get("mcp-session-id").is_some());

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject batch initialize request
#[tokio::test]
async fn should_reject_batch_initialize_request() {
    let server_options = HyperServerOptions {
        port: random_port(),
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        enable_json_response: None,
        ..Default::default()
    };

    let server = create_start_server(server_options).await;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let first_init_message = ClientJsonrpcRequest::new(
        RequestId::String("first-init".to_string()),
        initialize_request().into(),
    );
    let second_init_message = ClientJsonrpcRequest::new(
        RequestId::String("second-init".to_string()),
        initialize_request().into(),
    );

    let messages = vec![
        ClientMessage::Request(first_init_message),
        ClientMessage::Request(second_init_message),
    ];
    let batch_message = ClientMessages::Batch(messages);

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&batch_message).unwrap(),
        None,
        None,
    )
    .await
    .expect("Request failed");

    let error_data: SdkError = response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::INVALID_REQUEST as i64);
    assert!(error_data
        .message
        .contains("Only one initialization request is allowed"));
}

// should handle post requests via sse response correctly
#[tokio::test]
async fn should_handle_post_requests_via_sse_response_correctly() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let events = read_sse_event(response, 1).await.unwrap();
    let message: ServerJsonrpcResponse = serde_json::from_str(&events[0].2).unwrap();

    assert!(matches!(message.id, RequestId::Integer(1)));

    let ResultFromServer::ServerResult(ServerResult::ListToolsResult(result)) = message.result
    else {
        panic!("invalid ListToolsResult")
    };

    assert_eq!(result.tools.len(), 1);

    let tool = &result.tools[0];
    assert_eq!(tool.name, "say_hello");
    assert_eq!(
        tool.description.as_ref().unwrap(),
        r#"Accepts a person's name and says a personalized "Hello" to that person"#
    );

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should call a tool and return the result
#[tokio::test]
async fn should_call_a_tool_and_return_the_result() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let mut map = Map::new();
    map.insert("name".to_string(), Value::String("Ali".to_string()));

    let json_rpc_message: ClientJsonrpcRequest = ClientJsonrpcRequest::new(
        RequestId::Integer(1),
        CallToolRequest::new(CallToolRequestParams {
            arguments: Some(map),
            name: "say_hello".to_string(),
        })
        .into(),
    );

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let events = read_sse_event(response, 1).await.unwrap();
    let message: ServerJsonrpcResponse = serde_json::from_str(&events[0].2).unwrap();

    assert!(matches!(message.id, RequestId::Integer(1)));

    let ResultFromServer::ServerResult(ServerResult::CallToolResult(result)) = message.result
    else {
        panic!("invalid CallToolResult")
    };

    assert_eq!(result.content.len(), 1);
    assert_eq!(
        result.content[0].as_text_content().unwrap().text,
        "Hello, Ali!"
    );

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject requests without a valid session ID
#[tokio::test]
async fn should_reject_requests_without_a_valid_session_id() {
    let (server, _session_id) = initialize_server(None).await.unwrap();

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None, // pass no session id
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error_data: SdkError = response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject invalid session ID
#[tokio::test]
async fn should_reject_invalid_session_id() {
    let (server, _session_id) = initialize_server(None).await.unwrap();

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some("invalid-session-id"),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let error_data: SdkError = response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::SESSION_NOT_FOUND as i64); // Typescript sdk uses -32001 code

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

async fn get_standalone_stream(
    streamable_url: &str,
    session_id: &str,
    last_event_id: Option<&str>,
) -> reqwest::Response {
    let mut headers = HashMap::new();
    headers.insert("Accept", "text/event-stream , application/json");
    headers.insert("mcp-session-id", session_id);
    headers.insert("mcp-protocol-version", "2025-03-26");

    if let Some(last_event_id) = last_event_id {
        headers.insert("last-event-id", last_event_id);
    }

    let response = send_get_request(streamable_url, Some(headers))
        .await
        .unwrap();
    response
}

// should establish standalone SSE stream and receive server-initiated messages
#[tokio::test]
async fn should_establish_standalone_stream_and_receive_server_messages() {
    let (server, session_id) = initialize_server(None).await.unwrap();
    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;

    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        response
            .headers()
            .get("mcp-session-id")
            .unwrap()
            .to_str()
            .unwrap(),
        session_id
    );

    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/event-stream"
    );

    // Send a notification (server-initiated message) that should appear on SSE stream
    server
        .hyper_runtime
        .send_logging_message(
            &session_id,
            LoggingMessageNotificationParams {
                data: json!("Test notification"),
                level: rust_mcp_schema::LoggingLevel::Info,
                logger: None,
            },
        )
        .await
        .unwrap();

    let events = read_sse_event(response, 1).await.unwrap();
    let message: ServerJsonrpcNotification = serde_json::from_str(&events[0].2).unwrap();

    let NotificationFromServer::ServerNotification(ServerNotification::LoggingMessageNotification(
        notification,
    )) = message.notification
    else {
        panic!("invalid message received!");
    };

    assert_eq!(notification.params.level, LoggingLevel::Info);
    assert_eq!(
        notification.params.data.as_str().unwrap(),
        "Test notification"
    );

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should establish standalone SSE stream and receive server-initiated requests
#[tokio::test]
async fn should_establish_standalone_stream_and_receive_server_requests() {
    let (server, session_id) = initialize_server(None).await.unwrap();
    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;

    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        response
            .headers()
            .get("mcp-session-id")
            .unwrap()
            .to_str()
            .unwrap(),
        session_id
    );

    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/event-stream"
    );

    let hyper_server = Arc::new(server.hyper_runtime);

    // Send two server-initiated request that should appear on SSE stream with a valid request_id
    for _ in 0..2 {
        let hyper_server_clone = hyper_server.clone();
        let session_id_clone = session_id.to_string();
        tokio::spawn(async move {
            hyper_server_clone
                .list_roots(&session_id_clone, None)
                .await
                .unwrap();
        });
    }

    for i in 0..2 {
        // send responses back to the server for two server initiated requests
        let json_rpc_message: ClientJsonrpcResponse = ClientJsonrpcResponse::new(
            RequestId::Integer(i),
            ListRootsResult {
                meta: None,
                roots: vec![],
            }
            .into(),
        );
        send_post_request(
            &server.streamable_url,
            &serde_json::to_string(&json_rpc_message).unwrap(),
            Some(&session_id),
            None,
        )
        .await
        .expect("Request failed");
    }

    // read two events from the sse stream
    let events = read_sse_event(response, 2).await.unwrap();

    let message1: ServerJsonrpcRequest = serde_json::from_str(&events[0].2).unwrap();

    let RequestFromServer::ServerRequest(ServerRequest::ListRootsRequest(_)) = message1.request
    else {
        panic!("invalid message received!");
    };

    let message2: ServerJsonrpcRequest = serde_json::from_str(&events[1].2).unwrap();

    let RequestFromServer::ServerRequest(ServerRequest::ListRootsRequest(_)) = message1.request
    else {
        panic!("invalid message received!");
    };

    // ensure request_ids are unique
    assert!(message2.id != message1.id);

    hyper_server.graceful_shutdown(ONE_MILLISECOND);
}

// should not close GET SSE stream after sending multiple server notifications
#[tokio::test]
async fn should_not_close_get_sse_stream() {
    let (server, session_id) = initialize_server(None).await.unwrap();
    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;

    assert_eq!(response.status(), StatusCode::OK);

    server
        .hyper_runtime
        .send_logging_message(
            &session_id,
            LoggingMessageNotificationParams {
                data: json!("First notification"),
                level: rust_mcp_schema::LoggingLevel::Info,
                logger: None,
            },
        )
        .await
        .unwrap();

    let mut stream = response.bytes_stream();
    let event = read_sse_event_from_stream(&mut stream, 1).await.unwrap()[0].clone();
    let message: ServerJsonrpcNotification = serde_json::from_str(&event.2).unwrap();

    let NotificationFromServer::ServerNotification(ServerNotification::LoggingMessageNotification(
        notification,
    )) = message.notification
    else {
        panic!("invalid message received!");
    };

    assert_eq!(notification.params.level, LoggingLevel::Info);
    assert_eq!(
        notification.params.data.as_str().unwrap(),
        "First notification"
    );

    server
        .hyper_runtime
        .send_logging_message(
            &session_id,
            LoggingMessageNotificationParams {
                data: json!("Second notification"),
                level: rust_mcp_schema::LoggingLevel::Info,
                logger: None,
            },
        )
        .await
        .unwrap();

    let event = read_sse_event_from_stream(&mut stream, 1).await.unwrap()[0].clone();
    let message: ServerJsonrpcNotification = serde_json::from_str(&event.2).unwrap();

    let NotificationFromServer::ServerNotification(ServerNotification::LoggingMessageNotification(
        notification_2,
    )) = message.notification
    else {
        panic!("invalid message received!");
    };

    assert_eq!(notification_2.params.level, LoggingLevel::Info);
    assert_eq!(
        notification_2.params.data.as_str().unwrap(),
        "Second notification"
    );

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

//should reject second SSE stream for the same session
#[tokio::test]
async fn should_reject_second_sse_stream_for_the_same_session() {
    let (server, session_id) = initialize_server(None).await.unwrap();
    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let second_response = get_standalone_stream(&server.streamable_url, &session_id, None).await;
    assert_eq!(second_response.status(), StatusCode::CONFLICT);

    let error_data: SdkError = second_response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject GET requests without Accept: text/event-stream header
#[tokio::test]
async fn should_reject_get_requests() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("Accept", "application/json");
    headers.insert("mcp-session-id", &session_id);
    headers.insert("mcp-protocol-version", "2025-03-26");

    let response = send_get_request(&server.streamable_url, Some(headers))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE); //406
    let error_data: SdkError = response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code
    assert!(error_data.message.contains("must accept text/event-stream"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject POST requests without proper Accept header
#[tokio::test]
async fn reject_post_requests_without_accept_header() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let mut headers = HashMap::new();
    headers.insert("Accept", "application/json");
    headers.insert("mcp-session-id", &session_id);
    headers.insert("mcp-protocol-version", "2025-03-26");

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE); //406

    let error_data: SdkError = response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code
    assert!(error_data
        .message
        .contains("must accept both application/json and text/event-stream"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

//should reject unsupported Content-Type
#[tokio::test]
async fn should_reject_unsupported_content_type() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "text/plain");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("mcp-session-id", &session_id);
    headers.insert("mcp-protocol-version", "2025-03-26");

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE); //415

    let error_data: SdkError = response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code
    assert!(error_data
        .message
        .contains("Content-Type must be application/json"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should handle JSON-RPC batch notification messages with 202 response
#[tokio::test]
async fn should_handle_batch_notification_messages_with_202_response() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let batch_notification = ClientMessages::Batch(vec![
        ClientMessage::from_message(RootsListChangedNotification::new(None), None).unwrap(),
        ClientMessage::from_message(RootsListChangedNotification::new(None), None).unwrap(),
    ]);

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&batch_notification).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

// should properly handle invalid JSON data
#[tokio::test]
async fn should_properly_handle_invalid_json_data() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let response = send_post_request(
        &server.streamable_url,
        "This is not a valid JSON",
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let error_data: SdkError = response.json().await.unwrap();
    assert_eq!(error_data.code, SdkErrorCodes::PARSE_ERROR as i64);
    assert!(error_data.message.contains("Parse Error"));
}

// should send response messages to the connection that sent the request
#[tokio::test]
async fn should_send_response_messages_to_the_connection_that_sent_the_request() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let json_rpc_message_1: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let mut map = Map::new();
    map.insert("name".to_string(), Value::String("Ali".to_string()));

    let json_rpc_message_2: ClientJsonrpcRequest = ClientJsonrpcRequest::new(
        RequestId::Integer(1),
        CallToolRequest::new(CallToolRequestParams {
            arguments: Some(map),
            name: "say_hello".to_string(),
        })
        .into(),
    );

    let response_1 = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message_1).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    let response_2 = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message_2).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response_1.status(), StatusCode::OK);
    assert_eq!(response_2.status(), StatusCode::OK);

    let events = read_sse_event(response_2, 1).await.unwrap();
    let message: ServerJsonrpcResponse = serde_json::from_str(&events[0].2).unwrap();

    assert!(matches!(message.id, RequestId::Integer(1)));

    let ResultFromServer::ServerResult(ServerResult::CallToolResult(result)) = message.result
    else {
        panic!("invalid CallToolResult")
    };

    assert_eq!(result.content.len(), 1);
    assert_eq!(
        result.content[0].as_text_content().unwrap().text,
        "Hello, Ali!"
    );

    let events = read_sse_event(response_1, 1).await.unwrap();
    let message: ServerJsonrpcResponse = serde_json::from_str(&events[0].2).unwrap();

    assert!(matches!(message.id, RequestId::Integer(1)));

    let ResultFromServer::ServerResult(ServerResult::ListToolsResult(result)) = message.result
    else {
        panic!("invalid ListToolsResult")
    };

    assert_eq!(result.tools.len(), 1);

    let tool = &result.tools[0];
    assert_eq!(tool.name, "say_hello");
    assert_eq!(
        tool.description.as_ref().unwrap(),
        r#"Accepts a person's name and says a personalized "Hello" to that person"#
    );

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should properly handle DELETE requests and close session
#[tokio::test]
async fn should_properly_handle_delete_requests_and_close_session() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "text/plain");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("mcp-session-id", &session_id);
    headers.insert("mcp-protocol-version", "2025-03-26");

    let response = send_delete_request(&server.streamable_url, Some(&session_id), Some(headers))
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject DELETE requests with invalid session ID
#[tokio::test]
async fn should_reject_delete_requests_with_invalid_session_id() {
    let (server, _session_id) = initialize_server(None).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "text/plain");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("mcp-session-id", "invalid-session-id");
    headers.insert("mcp-protocol-version", "2025-03-26");

    let response = send_delete_request(
        &server.streamable_url,
        Some("invalid-session-id"),
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let error_data: SdkError = response.json().await.unwrap();

    assert_eq!(error_data.code, SdkErrorCodes::SESSION_NOT_FOUND as i64); // Typescript sdk uses -32001 code
    assert!(error_data.message.contains("Session not found"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

/**
 * protocol version header validation
 */

// should accept requests without protocol version header
#[tokio::test]
async fn should_accept_requests_without_protocol_version_header() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject requests with unsupported protocol version
#[tokio::test]
async fn should_reject_requests_with_unsupported_protocol_version() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("mcp-protocol-version", "1999-15-21");

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error_data: SdkError = response.json().await.unwrap();

    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code
    assert!(error_data.message.contains("Unsupported protocol version"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should handle protocol version validation for get requests
#[tokio::test]
async fn should_handle_protocol_version_validation_for_get_requests() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("mcp-protocol-version", "1999-15-21");
    headers.insert("mcp-session-id", &session_id);

    let response = send_get_request(&server.streamable_url, Some(headers))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error_data: SdkError = response.json().await.unwrap();

    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code
    assert!(error_data.message.contains("Unsupported protocol version"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should handle protocol version validation for DELETE requests
#[tokio::test]
async fn should_handle_protocol_version_validation_for_delete_requests() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("mcp-protocol-version", "1999-15-21");

    let response = send_delete_request(&server.streamable_url, Some(&session_id), Some(headers))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error_data: SdkError = response.json().await.unwrap();

    assert_eq!(error_data.code, SdkErrorCodes::BAD_REQUEST as i64); // Typescript sdk uses -32000 code
    assert!(error_data.message.contains("Unsupported protocol version"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

/**
 * Test JSON Response Mode
 */

// should return JSON response for a single request
#[tokio::test]
async fn should_return_json_response_for_a_single_request() {
    let (server, session_id) = initialize_server(Some(true)).await.unwrap();

    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(1), ListToolsRequest::new(None).into());

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );
    assert!(response.headers().get("mcp-session-id").is_some());

    let message = response.json::<ServerJsonrpcResponse>().await.unwrap();

    let ResultFromServer::ServerResult(ServerResult::ListToolsResult(result)) = message.result
    else {
        panic!("invalid ListToolsResult")
    };

    assert_eq!(result.tools.len(), 1);

    let tool = &result.tools[0];
    assert_eq!(tool.name, "say_hello");
    assert_eq!(
        tool.description.as_ref().unwrap(),
        r#"Accepts a person's name and says a personalized "Hello" to that person"#
    );

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should return JSON response for batch requests
#[tokio::test]
async fn should_return_json_response_for_a_batch_request() {
    let (server, session_id) = initialize_server(Some(true)).await.unwrap();

    let json_rpc_message_1: ClientJsonrpcRequest = ClientJsonrpcRequest::new(
        RequestId::String("req_1".to_string()),
        ListToolsRequest::new(None).into(),
    );

    let mut map = Map::new();
    map.insert("name".to_string(), Value::String("Ali".to_string()));
    let json_rpc_message_3: ClientJsonrpcRequest = ClientJsonrpcRequest::new(
        RequestId::String("req_2".to_string()),
        CallToolRequest::new(CallToolRequestParams {
            arguments: Some(map),
            name: "say_hello".to_string(),
        })
        .into(),
    );

    let batch_message = ClientMessages::Batch(vec![
        json_rpc_message_1.into(),
        ClientMessage::from_message(RootsListChangedNotification::new(None), None).unwrap(),
        json_rpc_message_3.into(),
    ]);

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&batch_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );
    assert!(response.headers().get("mcp-session-id").is_some());

    let messages = response.json::<ServerMessages>().await.unwrap();

    let ServerMessages::Batch(mut messages) = messages else {
        panic!("Invalid message type");
    };

    assert_eq!(messages.len(), 2);

    let mut results = messages.drain(0..);
    let result_1 = results.next().unwrap();
    assert_eq!(
        result_1.request_id().unwrap(),
        RequestId::String("req_1".to_string())
    );
    let ResultFromServer::ServerResult(ServerResult::ListToolsResult(_)) =
        result_1.as_response().unwrap().result
    else {
        panic!("Expected a ListToolsResult");
    };

    let result_2 = results.next().unwrap();
    assert_eq!(
        result_2.request_id().unwrap(),
        RequestId::String("req_2".to_string())
    );
    let ResultFromServer::ServerResult(ServerResult::CallToolResult(_)) =
        result_2.as_response().unwrap().result
    else {
        panic!("Expected a CallToolResult");
    };

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should handle batch request messages with SSE stream for responses
#[tokio::test]
async fn should_handle_batch_request_messages_with_sse_stream_for_responses() {
    let (server, session_id) = initialize_server(None).await.unwrap();

    let json_rpc_message_1: ClientJsonrpcRequest = ClientJsonrpcRequest::new(
        RequestId::String("req_1".to_string()),
        ListToolsRequest::new(None).into(),
    );

    let mut map = Map::new();
    map.insert("name".to_string(), Value::String("Ali".to_string()));
    let json_rpc_message_2: ClientJsonrpcRequest = ClientJsonrpcRequest::new(
        RequestId::String("req_2".to_string()),
        CallToolRequest::new(CallToolRequestParams {
            arguments: Some(map),
            name: "say_hello".to_string(),
        })
        .into(),
    );

    let batch_message =
        ClientMessages::Batch(vec![json_rpc_message_1.into(), json_rpc_message_2.into()]);

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&batch_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let events = read_sse_event(response, 1).await.unwrap();
    let message: ServerMessages = serde_json::from_str(&events[0].2).unwrap();

    let ServerMessages::Batch(mut messages) = message else {
        panic!("Invalid message type");
    };

    assert_eq!(messages.len(), 2);

    let mut results = messages.drain(0..);
    let result_1 = results.next().unwrap();
    assert_eq!(
        result_1.request_id().unwrap(),
        RequestId::String("req_1".to_string())
    );
    let ResultFromServer::ServerResult(ServerResult::ListToolsResult(_)) =
        result_1.as_response().unwrap().result
    else {
        panic!("Expected a ListToolsResult");
    };

    let result_2 = results.next().unwrap();
    assert_eq!(
        result_2.request_id().unwrap(),
        RequestId::String("req_2".to_string())
    );
    let ResultFromServer::ServerResult(ServerResult::CallToolResult(_)) =
        result_2.as_response().unwrap().result
    else {
        panic!("Expected a CallToolResult");
    };
}

// Test DNS rebinding protection

// should accept requests with allowed host headers
#[tokio::test]
async fn should_accept_requests_with_allowed_host_headers() {
    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(0), initialize_request().into());

    let server_options = HyperServerOptions {
        port: 8090,
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        allowed_hosts: Some(vec!["127.0.0.1:8090".to_string()]),
        dns_rebinding_protection: true,
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    tokio::time::sleep(Duration::from_millis(250)).await;
    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None,
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert!(response.headers().get("mcp-session-id").is_some());

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject requests with disallowed host headers
#[tokio::test]
async fn should_reject_requests_with_disallowed_host_headers() {
    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(0), initialize_request().into());

    let server_options = HyperServerOptions {
        port: random_port(),
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        allowed_hosts: Some(vec!["example.com:3001".to_string()]),
        dns_rebinding_protection: true,
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    tokio::time::sleep(Duration::from_millis(250)).await;
    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None,
        None,
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error_data: SdkError = response.json().await.unwrap();
    assert!(error_data.message.contains("Invalid Host header"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should reject GET requests with disallowed host headers
#[tokio::test]
async fn should_reject_get_requests_with_disallowed_host_headers() {
    let server_options = HyperServerOptions {
        port: random_port(),
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        allowed_hosts: Some(vec!["example.com:3001".to_string()]),
        dns_rebinding_protection: true,
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");

    let response = send_get_request(&server.streamable_url, Some(headers))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error_data: SdkError = response.json().await.unwrap();
    assert!(error_data.message.contains("Invalid Host header"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should accept requests with allowed origin headers
#[tokio::test]
async fn should_accept_requests_with_allowed_origin_headers() {
    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(0), initialize_request().into());

    let server_options = HyperServerOptions {
        port: 3000,
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        allowed_origins: Some(vec![
            "http://localhost:3000".to_string(),
            "https://example.com".to_string(),
        ]),
        dns_rebinding_protection: true,
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    // Origin: "http://localhost:3000",

    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("Origin", "http://localhost:3000");

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None,
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert!(response.headers().get("mcp-session-id").is_some());

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

//should reject requests with disallowed origin headers
#[tokio::test]
async fn should_reject_requests_with_disallowed_origin_headers() {
    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(0), initialize_request().into());

    let server_options = HyperServerOptions {
        port: 3000,
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        allowed_origins: Some(vec!["http://localhost:3000".to_string()]),
        dns_rebinding_protection: true,
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("Origin", "http://evil.com");

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None,
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let error_data: SdkError = response.json().await.unwrap();

    assert!(error_data.message.contains("Invalid Origin header"));

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should skip all validations when enableDnsRebindingProtection is false
#[tokio::test]
async fn should_skip_all_validations_when_false() {
    let json_rpc_message: ClientJsonrpcRequest =
        ClientJsonrpcRequest::new(RequestId::Integer(0), initialize_request().into());

    let server_options = HyperServerOptions {
        port: 3030,
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        allowed_hosts: Some(vec!["localhost".to_string()]),
        allowed_origins: Some(vec!["http://localhost:3030".to_string()]),
        dns_rebinding_protection: false,
        ..Default::default()
    };

    let server = create_start_server(server_options).await;

    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut headers = HashMap::new();
    headers.insert("Content-Type", "application/json");
    headers.insert("Accept", "application/json, text/event-stream");
    headers.insert("Origin", "http://evil.com");
    headers.insert("Host", "evil.com");

    let response = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        None,
        Some(headers),
    )
    .await
    .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

// should store and include event IDs in server SSE messages
#[tokio::test]
async fn should_store_and_include_event_ids_in_server_sse_messages() {
    common::init_tracing();
    let (server, session_id) = initialize_server(Some(true)).await.unwrap();
    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;

    assert_eq!(response.status(), StatusCode::OK);

    let _ = server
        .hyper_runtime
        .send_logging_message(
            &session_id,
            LoggingMessageNotificationParams {
                data: json!("notification1"),
                level: LoggingLevel::Info,
                logger: None,
            },
        )
        .await;

    let _ = server
        .hyper_runtime
        .send_logging_message(
            &session_id,
            LoggingMessageNotificationParams {
                data: json!("notification2"),
                level: LoggingLevel::Info,
                logger: None,
            },
        )
        .await;

    // read two events
    let events = read_sse_event(response, 2).await.unwrap();
    assert_eq!(events.len(), 2);
    // verify we got the notification with an event ID
    let (first_id, _, data) = events[0].clone();
    let (second_id, _, _) = events[0].clone();

    let message: ServerJsonrpcNotification = serde_json::from_str(&data).unwrap();

    let NotificationFromServer::ServerNotification(ServerNotification::LoggingMessageNotification(
        notification1,
    )) = message.notification
    else {
        panic!("invalid message received!");
    };

    assert_eq!(notification1.params.data.as_str().unwrap(), "notification1");

    let first_id = first_id.unwrap();
    assert!(second_id.is_some());

    //messages should be stored and accessible
    let events = server
        .event_store
        .unwrap()
        .events_after(first_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(events.messages.len(), 1);

    // deserialize the message returned by event_store
    let message: ServerJsonrpcNotification = serde_json::from_str(&events.messages[0]).unwrap();
    let NotificationFromServer::ServerNotification(ServerNotification::LoggingMessageNotification(
        notification2,
    )) = message.notification
    else {
        panic!("invalid message in store!");
    };
    assert_eq!(notification2.params.data.as_str().unwrap(), "notification2");
}

// should store and replay MCP server tool notifications
#[tokio::test]
async fn should_store_and_replay_mcp_server_tool_notifications() {
    common::init_tracing();
    let (server, session_id) = initialize_server(Some(true)).await.unwrap();
    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let _ = server
        .hyper_runtime
        .send_logging_message(
            &session_id,
            LoggingMessageNotificationParams {
                data: json!("notification1"),
                level: LoggingLevel::Info,
                logger: None,
            },
        )
        .await;

    let events = read_sse_event(response, 1).await.unwrap();
    assert_eq!(events.len(), 1);
    // verify we got the notification with an event ID
    let (first_id, _, data) = events[0].clone();

    let message: ServerJsonrpcNotification = serde_json::from_str(&data).unwrap();

    let NotificationFromServer::ServerNotification(ServerNotification::LoggingMessageNotification(
        notification1,
    )) = message.notification
    else {
        panic!("invalid message received!");
    };

    assert_eq!(notification1.params.data.as_str().unwrap(), "notification1");

    let first_id = first_id.unwrap();

    // sse connection is closed in read_sse_event()
    // wait so server detect the disconnect and simulate a network error
    tokio::time::sleep(Duration::from_secs(3)).await;
    tokio::task::yield_now().await;
    // we send another notification while SSE is disconnected
    let _result = server
        .hyper_runtime
        .send_logging_message(
            &session_id,
            LoggingMessageNotificationParams {
                data: json!("notification2"),
                level: LoggingLevel::Info,
                logger: None,
            },
        )
        .await;

    //  make a new standalone SSE connection to simulate a re-connection
    let response =
        get_standalone_stream(&server.streamable_url, &session_id, Some(&first_id)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let events = read_sse_event(response, 1).await.unwrap();

    assert_eq!(events.len(), 1);
    let message: ServerJsonrpcNotification = serde_json::from_str(&events[0].2).unwrap();

    let NotificationFromServer::ServerNotification(ServerNotification::LoggingMessageNotification(
        notification1,
    )) = message.notification
    else {
        panic!("invalid message received!");
    };

    assert_eq!(notification1.params.data.as_str().unwrap(), "notification2");
}

// should return 400 error for invalid JSON-RPC messages
// should keep stream open after sending server notifications
// NA: should reject second initialization request
// NA: should pass request info to tool callback
// NA: should reject second SSE stream even in stateless mode
// should reject requests to uninitialized server
// should accept requests with matching protocol version
// should accept when protocol version differs from negotiated version
// should call a tool with authInfo
// should calls tool without authInfo when it is optional
// should accept pre-parsed request body
// should handle pre-parsed batch messages
// should prefer pre-parsed body over request body
// should operate without session ID validation
// should handle POST requests with various session IDs in stateless mode
// should call onsessionclosed callback when session is closed via DELETE
// should not call onsessionclosed callback when not provided
// should not call onsessionclosed callback for invalid session DELETE
// should call onsessionclosed callback with correct session ID when multiple sessions exist
// should support async onsessioninitialized callback
// should support sync onsessioninitialized callback (backwards compatibility)
// should support async onsessionclosed callback
// should propagate errors from async onsessioninitialized callback
// should propagate errors from async onsessionclosed callback
// should handle both async callbacks together
// should validate both host and origin when both are configured
