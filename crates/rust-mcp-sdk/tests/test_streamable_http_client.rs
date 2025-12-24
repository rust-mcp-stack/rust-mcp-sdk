#[path = "common/common.rs"]
pub mod common;

use crate::common::{
    create_sse_response, debug_wiremock, random_port,
    test_client_common::{
        initialize_client, InitializedClient, INITIALIZE_REQUEST, TEST_SESSION_ID,
    },
    test_server_common::{
        create_start_server, LaunchedServer, TestIdGenerator, INITIALIZE_RESPONSE,
    },
    wait_for_n_requests, wiremock_request, MockBuilder, SimpleMockServer, SseEvent,
};
use common::test_client_common::create_client;
use hyper::{Method, StatusCode};
use rust_mcp_schema::{
    schema_utils::{
        ClientJsonrpcRequest, ClientMessage, CustomRequest, MessageFromServer, RequestFromClient,
        RequestFromServer, ResultFromServer, RpcMessage, ServerMessage,
    },
    RequestId,
};
use rust_mcp_sdk::{
    error::McpSdkError, mcp_server::HyperServerOptions, McpClient, TransportError,
    MCP_LAST_EVENT_ID_HEADER,
};
use serde_json::{json, Map, Value};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};
use wiremock::{
    http::{HeaderName, HeaderValue},
    matchers::{body_json_string, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

// should send JSON-RPC messages via POST
#[tokio::test]
async fn should_send_json_rpc_messages_via_post() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // initialize response
    let response = create_sse_response(INITIALIZE_RESPONSE);

    // initialize request and response
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(INITIALIZE_REQUEST))
        .respond_with(response)
        .expect(1)
        .mount(&mock_server)
        .await;

    // receive initialized notification
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    let mcp_url = format!("{}/mcp", mock_server.uri());
    let (client, _) = create_client(&mcp_url, None).await;

    client.clone().start().await.unwrap();

    let received_request = wiremock_request(&mock_server, 0).await;
    let header_values = received_request
        .headers
        .get(&HeaderName::from_str("accept").unwrap())
        .unwrap();

    assert!(header_values.contains(&HeaderValue::from_str("application/json").unwrap()));
    assert!(header_values.contains(&HeaderValue::from_str("text/event-stream").unwrap()));

    wait_for_n_requests(&mock_server, 2, None).await;
}

// should send batch messages
#[tokio::test]
async fn should_send_batch_messages() {
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(None, None).await;

    let response = create_sse_response(
        r#"[{"id":"id1","jsonrpc":"2.0", "result":{}},{"id":"id2","jsonrpc":"2.0", "result":{}}]"#,
    );

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(response)
        // .expect(1)
        .mount(&mock_server)
        .await;

    let message_1: ClientMessage = ClientJsonrpcRequest::new(
        RequestId::String("id1".to_string()),
        RequestFromClient::CustomRequest(CustomRequest {
            method: "test1".to_string(),
            params: Some(Map::new()),
        }),
    )
    .into();
    let message_2: ClientMessage = ClientJsonrpcRequest::new(
        RequestId::String("id2".to_string()),
        RequestFromClient::CustomRequest(CustomRequest {
            method: "test2".to_string(),
            params: Some(Map::new()),
        }),
    )
    .into();

    let result = client
        .send_batch(vec![message_1, message_2], None)
        .await
        .unwrap()
        .unwrap();

    // two results for two requests
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| {
        let id = r.request_id().unwrap();
        id == RequestId::String("id1".to_string()) || id == RequestId::String("id2".to_string())
    }));

    // not an Error
    assert!(result
        .iter()
        .all(|r| matches!(r, ServerMessage::Response(_))));

    // debug_wiremock(&mock_server).await;
}

// should store session ID received during initialization
#[tokio::test]
async fn should_store_session_id_received_during_initialization() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // initialize response
    let response =
        create_sse_response(INITIALIZE_RESPONSE).append_header("mcp-session-id", "test-session-id");

    // initialize request and response
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(INITIALIZE_REQUEST))
        .respond_with(response)
        .expect(1)
        .mount(&mock_server)
        .await;

    // receive initialized notification
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ))
        .and(header("mcp-session-id", "test-session-id"))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    let mcp_url = format!("{}/mcp", mock_server.uri());
    let (client, _) = create_client(&mcp_url, None).await;

    client.clone().start().await.unwrap();

    let received_request = wiremock_request(&mock_server, 0).await;
    let header_values = received_request
        .headers
        .get(&HeaderName::from_str("accept").unwrap())
        .unwrap();

    assert!(header_values.contains(&HeaderValue::from_str("application/json").unwrap()));
    assert!(header_values.contains(&HeaderValue::from_str("text/event-stream").unwrap()));

    wait_for_n_requests(&mock_server, 2, None).await;
}

// should terminate session with DELETE request
#[tokio::test]
async fn should_terminate_session_with_delete_request() {
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(Some(TEST_SESSION_ID.to_string()), None).await;

    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .and(header("mcp-session-id", "test-session-id"))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    client.terminate_session().await;
}

// should handle 405 response when server doesn't support session termination
#[tokio::test]
async fn should_handle_405_unsupported_session_termination() {
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(Some(TEST_SESSION_ID.to_string()), None).await;

    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .and(header("mcp-session-id", "test-session-id"))
        .respond_with(ResponseTemplate::new(405))
        .expect(1)
        .mount(&mock_server)
        .await;

    client.terminate_session().await;
}

// should handle 404 response when session expires
#[tokio::test]
async fn should_handle_404_response_when_session_expires() {
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(Some(TEST_SESSION_ID.to_string()), None).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = client.ping(None, None).await;

    matches!(
        result,
        Err(McpSdkError::Transport(TransportError::SessionExpired))
    );
}

// should handle non-streaming JSON response
#[tokio::test]
async fn should_handle_non_streaming_json_response() {
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(Some(TEST_SESSION_ID.to_string()), None).await;

    let response = ResponseTemplate::new(200)
        .set_body_json(json!({
            "id":1,"jsonrpc":"2.0", "result":{"something":"good"}
        }))
        .insert_header("Content-Type", "application/json");

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(response)
        .expect(1)
        .mount(&mock_server)
        .await;

    let request = RequestFromClient::CustomRequest(CustomRequest {
        method: "test1".to_string(),
        params: Some(Map::new()),
    });

    let result = client.request(request, None).await.unwrap();

    let ResultFromServer::Result(result) = result else {
        panic!("Wrong result variant!")
    };

    let extra = result.extra.unwrap();
    assert_eq!(extra.get("something").unwrap(), "good");
}

// should handle successful initial GET connection for SSE
#[tokio::test]
async fn should_handle_successful_initial_get_connection_for_sse() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // initialize response
    let response = create_sse_response(INITIALIZE_RESPONSE);

    // initialize request and response
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(INITIALIZE_REQUEST))
        .respond_with(response)
        .expect(1)
        .mount(&mock_server)
        .await;

    // receive initialized notification
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    // let payload = r#"{"jsonrpc": "2.0", "method": "serverNotification", "params": {}}"#;
    //
    let mut body = String::new();
    body.push_str("data: Connection established\n\n");

    let response = ResponseTemplate::new(200)
        .set_body_raw(body.into_bytes(), "text/event-stream")
        .append_header("Connection", "keep-alive");

    // Mount the mock for a GET request
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(response)
        .mount(&mock_server)
        .await;

    let mcp_url = format!("{}/mcp", mock_server.uri());
    let (client, _) = create_client(&mcp_url, None).await;

    client.clone().start().await.unwrap();

    let requests = mock_server.received_requests().await.unwrap();
    let get_request = requests
        .iter()
        .find(|r| r.method == wiremock::http::Method::Get);

    assert!(get_request.is_some())
}

#[tokio::test]
async fn should_receive_server_initiated_messaged() {
    let server_options = HyperServerOptions {
        port: random_port(),
        session_id_generator: Some(Arc::new(TestIdGenerator::new(vec![
            "AAA-BBB-CCC".to_string()
        ]))),
        enable_json_response: Some(false),
        ..Default::default()
    };
    let LaunchedServer {
        hyper_runtime,
        streamable_url,
        sse_url,
        sse_message_url,
        event_store,
    } = create_start_server(server_options).await;

    let (client, message_history) = create_client(&streamable_url, None).await;

    client.clone().start().await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let result = hyper_runtime
        .ping(&"AAA-BBB-CCC".to_string(), None, None)
        .await
        .unwrap();

    let lock = message_history.read().await;
    let ping_request = lock
        .iter()
        .find(|m| {
            matches!(
                m,
                MessageFromServer::RequestFromServer(RequestFromServer::PingRequest(_))
            )
        })
        .unwrap();
    let MessageFromServer::RequestFromServer(RequestFromServer::PingRequest(_)) = ping_request
    else {
        panic!("Request is not a match!")
    };
    assert!(result.meta.is_some());

    let v = result.meta.unwrap().get("meta_number").unwrap().clone();

    assert!(matches!(v, Value::Number(value) if value.as_i64().unwrap()==1515)) //1515 is passed from TestClientHandler
}

// should attempt initial GET connection and handle 405 gracefully
#[tokio::test]
async fn should_attempt_initial_get_connection_and_handle_405_gracefully() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // initialize response
    let response = create_sse_response(INITIALIZE_RESPONSE);

    // initialize request and response
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(INITIALIZE_REQUEST))
        .respond_with(response)
        .expect(1)
        .mount(&mock_server)
        .await;

    // Mount the mock for a GET request
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&mock_server)
        .await;

    // receive initialized notification
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    // let payload = r#"{"jsonrpc": "2.0", "method": "serverNotification", "params": {}}"#;
    //
    let mut body = String::new();
    body.push_str("data: Connection established\n\n");

    let _response = ResponseTemplate::new(405)
        .set_body_raw(body.into_bytes(), "text/event-stream")
        .append_header("Connection", "keep-alive");

    let mcp_url = format!("{}/mcp", mock_server.uri());
    let (client, _) = create_client(&mcp_url, None).await;

    client.clone().start().await.unwrap();

    let requests = mock_server.received_requests().await.unwrap();
    let get_request = requests
        .iter()
        .find(|r| r.method == wiremock::http::Method::Get);

    assert!(get_request.is_some());

    // send a batch message, runtime should work as expected with no issue

    let response = create_sse_response(
        r#"[{"id":"id1","jsonrpc":"2.0", "result":{}},{"id":"id2","jsonrpc":"2.0", "result":{}}]"#,
    );

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(response)
        // .expect(1)
        .mount(&mock_server)
        .await;

    let message_1: ClientMessage = ClientJsonrpcRequest::new(
        RequestId::String("id1".to_string()),
        RequestFromClient::CustomRequest(CustomRequest {
            method: "test1".to_string(),
            params: Some(Map::new()),
        }),
    )
    .into();
    let message_2: ClientMessage = ClientJsonrpcRequest::new(
        RequestId::String("id2".to_string()),
        RequestFromClient::CustomRequest(CustomRequest {
            method: "test2".to_string(),
            params: Some(Map::new()),
        }),
    )
    .into();

    let result = client
        .send_batch(vec![message_1, message_2], None)
        .await
        .unwrap()
        .unwrap();

    // two results for two requests
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| {
        let id = r.request_id().unwrap();
        id == RequestId::String("id1".to_string()) || id == RequestId::String("id2".to_string())
    }));
}

// should handle multiple concurrent SSE streams
#[tokio::test]
async fn should_handle_multiple_concurrent_sse_streams() {
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(None, None).await;

    let message_1: ClientMessage = ClientJsonrpcRequest::new(
        RequestId::String("id1".to_string()),
        RequestFromClient::CustomRequest(CustomRequest {
            method: "test1".to_string(),
            params: Some(Map::new()),
        }),
    )
    .into();
    let message_2: ClientMessage = ClientJsonrpcRequest::new(
        RequestId::String("id2".to_string()),
        RequestFromClient::CustomRequest(CustomRequest {
            method: "test2".to_string(),
            params: Some(Map::new()),
        }),
    )
    .into();

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|req: &wiremock::Request|  {
            let body_string = String::from_utf8(req.body.clone()).unwrap();
            if body_string.contains("test3") {
                create_sse_response(r#"{"id":1,"jsonrpc":"2.0", "result":{}}"#)
            } else {
                create_sse_response(
                    r#"[{"id":"id1","jsonrpc":"2.0", "result":{}},{"id":"id2","jsonrpc":"2.0", "result":{}}]"#,
                )
            }
        })
        .expect(2)
        .mount(&mock_server)
        .await;

    let message_3 = RequestFromClient::CustomRequest(CustomRequest {
        method: "test3".to_string(),
        params: Some(Map::new()),
    });
    let request1 = client.send_batch(vec![message_1, message_2], None);
    let request2 = client.send(message_3.into(), None, None);

    // Run them concurrently and wait for both
    let (res_batch, res_single) = tokio::join!(request1, request2);

    let res_batch = res_batch.unwrap().unwrap();
    // two results for two requests in the batch
    assert_eq!(res_batch.len(), 2);
    assert!(res_batch.iter().all(|r| {
        let id = r.request_id().unwrap();
        id == RequestId::String("id1".to_string()) || id == RequestId::String("id2".to_string())
    }));

    // not an Error
    assert!(res_batch
        .iter()
        .all(|r| matches!(r, ServerMessage::Response(_))));

    let res_single = res_single.unwrap().unwrap();
    let ServerMessage::Response(res_single) = res_single else {
        panic!("invalid respinse type, expected Result!")
    };

    assert!(matches!(res_single.id, RequestId::Integer(id) if id==1));
}

// should throw error when invalid content-type is received
#[tokio::test]
async fn should_throw_error_when_invalid_content_type_is_received() {
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(None, None).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"id":0,"jsonrpc":"2.0", "result":{}}"#.to_string().into_bytes(),
            "text/plain",
        ))
        .expect(1)
        .mount(&mock_server)
        .await;

    let result = client.ping(None, None).await;

    let Err(McpSdkError::Transport(TransportError::UnexpectedContentType(content_type))) = result
    else {
        panic!("Expected a TransportError::UnexpectedContentType error!");
    };

    assert_eq!(content_type, "text/plain");
}

// should always send specified custom headers
#[tokio::test]
async fn should_always_send_specified_custom_headers() {
    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "CustomValue".to_string());
    let InitializedClient {
        client,
        mcp_url: _,
        mock_server,
    } = initialize_client(None, Some(headers)).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"id":1,"jsonrpc":"2.0", "result":{}}"#.to_string().into_bytes(),
            "application/json",
        ))
        .expect(1)
        .mount(&mock_server)
        .await;

    let _result = client.ping(None, None).await;

    let requests = mock_server.received_requests().await.unwrap();

    assert_eq!(requests.len(), 4);
    assert!(requests
        .iter()
        .all(|r| r.headers.get(&"X-Custom-Header".into()).unwrap().as_str() == "CustomValue"));

    debug_wiremock(&mock_server).await
}

// should reconnect a GET-initiated notification stream that fails

#[tokio::test]
async fn should_reconnect_a_get_initiated_notification_stream_that_fails() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // initialize response
    let response = create_sse_response(INITIALIZE_RESPONSE);

    // initialize request and response
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(INITIALIZE_REQUEST))
        .respond_with(response)
        .expect(1)
        .mount(&mock_server)
        .await;

    // two GET Mock, each expects one call , first time it fails, second retry it succeeds
    let response = ResponseTemplate::new(502)
        .set_body_raw("".to_string().into_bytes(), "text/event-stream")
        .append_header("Connection", "keep-alive");

    // Mount the mock for a GET request
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(response)
        .expect(1)
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    let response = ResponseTemplate::new(200)
        .set_body_raw(
            "data: Connection established\n\n".to_string().into_bytes(),
            "text/event-stream",
        )
        .append_header("Connection", "keep-alive");
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(response)
        .expect(1)
        .mount(&mock_server)
        .await;

    // receive initialized notification
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_json_string(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    let mcp_url = format!("{}/mcp", mock_server.uri());
    let (client, _) = create_client(&mcp_url, None).await;

    client.clone().start().await.unwrap();
}

//****************** Resumability ******************
// should pass lastEventId when reconnecting
#[tokio::test]
async fn should_pass_last_event_id_when_reconnecting() {
    let msg = r#"{"jsonrpc":"2.0","method":"notifications/message","params":{"data":{},"level":"debug"}}"#;

    let mocks = vec![
        MockBuilder::new_sse(Method::POST, "/mcp".to_string(), INITIALIZE_RESPONSE).build(),
        MockBuilder::new_breakable_sse(
            Method::GET,
            "/mcp".to_string(),
            SseEvent {
                data: Some(msg.into()),
                event: Some("message".to_string()),
                id: None,
            },
            Duration::from_millis(100),
            5,
        )
        .expect(2)
        .build(),
        MockBuilder::new_sse(
            Method::POST,
            "/mcp".to_string(),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .build(),
    ];

    let (url, handle) = SimpleMockServer::start_with_mocks(mocks).await;
    let mcp_url = format!("{url}/mcp");

    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "CustomValue".to_string());
    let (client, _) = create_client(&mcp_url, Some(headers)).await;

    client.clone().start().await.unwrap();

    assert!(client.is_initialized());

    // give it time for re-connection
    tokio::time::sleep(Duration::from_secs(2)).await;

    let request_history = handle.get_history().await;

    let get_requests: Vec<_> = request_history
        .iter()
        .filter(|r| r.0.method == Method::GET)
        .collect();

    // there should be more than one GET reueat, indicating reconnection
    assert!(get_requests.len() > 1);

    let Some(last_get_request) = get_requests.last() else {
        panic!("Unable to find last GET request!");
    };

    let last_event_id = last_get_request
        .0
        .headers
        .get(axum::http::HeaderName::from_static(
            MCP_LAST_EVENT_ID_HEADER,
        ));

    // last-event-id should be sent
    assert!(
        matches!(last_event_id, Some(last_event_id) if last_event_id.to_str().unwrap().starts_with("msg-id"))
    );

    // custom headers should be passed for all GET requests
    assert!(get_requests.iter().all(|r| r
        .0
        .headers
        .get(axum::http::HeaderName::from_str("X-Custom-Header").unwrap())
        .unwrap()
        .to_str()
        .unwrap()
        == "CustomValue"));

    println!("last_event_id {:?} ", last_event_id.unwrap());
}

// should NOT reconnect a POST-initiated stream that fails
#[tokio::test]
async fn should_not_reconnect_a_post_initiated_stream_that_fails() {
    let mocks = vec![
        MockBuilder::new_sse(Method::POST, "/mcp".to_string(), INITIALIZE_RESPONSE)
            .expect(1)
            .build(),
        MockBuilder::new_sse(Method::GET, "/mcp".to_string(), "".to_string())
            .with_status(StatusCode::METHOD_NOT_ALLOWED)
            .build(),
        MockBuilder::new_sse(
            Method::POST,
            "/mcp".to_string(),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .expect(1)
        .build(),
        MockBuilder::new_breakable_sse(
            Method::POST,
            "/mcp".to_string(),
            SseEvent {
                data: Some("msg".to_string()),
                event: None,
                id: None,
            },
            Duration::ZERO,
            0,
        )
        .build(),
    ];

    let (url, handle) = SimpleMockServer::start_with_mocks(mocks).await;
    let mcp_url = format!("{url}/mcp");

    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "CustomValue".to_string());
    let (client, _) = create_client(&mcp_url, Some(headers)).await;

    client.clone().start().await.unwrap();

    assert!(client.is_initialized());

    let result = client.send_roots_list_changed(None).await;

    assert!(result.is_err());

    tokio::time::sleep(Duration::from_secs(2)).await;

    let request_history = handle.get_history().await;
    let post_requests: Vec<_> = request_history
        .iter()
        .filter(|r| r.0.method == Method::POST)
        .collect();
    assert_eq!(post_requests.len(), 3); // initialize, initialized, root_list_changed
}

//****************** Auth ******************
// attempts auth flow on 401 during POST request
// invalidates all credentials on InvalidClientError during auth
// invalidates all credentials on UnauthorizedClientError during auth
//invalidates tokens on InvalidGrantError during auth

//****************** Others ******************
// custom fetch in auth code paths
// should support custom reconnection options
// uses custom fetch implementation if provided
// should have exponential backoff with configurable maxRetries
