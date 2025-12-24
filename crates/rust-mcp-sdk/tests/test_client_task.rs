#[path = "common/common.rs"]
pub mod common;
mod test_streamable_http_client;

use std::{collections::HashMap, time::Duration};

use http::Method;
use rust_mcp_macros::{mcp_elicit, JsonSchema};
use rust_mcp_schema::{
    ElicitRequest, ElicitRequestFormParams, ElicitRequestParams, RequestId, TaskMetadata,
    TaskStatus,
};
use rust_mcp_sdk::{
    schema::{
        ClientJsonrpcNotification, ClientJsonrpcResponse, ClientMessage, MessageFromClient,
        MessageFromServer, NotificationFromClient, RequestFromClient, ResultFromClient,
        ResultFromServer, ServerJsonrpcRequest,
    },
    McpClient,
};

use crate::common::{
    debug_wiremock, init_tracing,
    test_client_common::{create_client, initialize_client, InitializedClient, TEST_SESSION_ID},
    test_server_common::INITIALIZE_RESPONSE,
    MockBuilder, SimpleMockServer, SseEvent,
};

#[mcp_elicit(message = "Please enter your info", mode = form)]
#[derive(JsonSchema)]
pub struct UserEmail {
    #[json_schema(title = "Email", format = "email")]
    pub email: Option<String>,
}

// // Sends a request to the client asking the user to provide input
// let result: ElicitResult = server.request_elicitation(UserInfo::elicit_request_params()).await?;

#[tokio::test]
async fn test_client_task_normal() {
    let elicit_params: ElicitRequestParams =
        UserEmail::elicit_request_params().with_task(TaskMetadata { ttl: Some(10000) });

    let elicit_request = ElicitRequest::new(RequestId::Integer(1), elicit_params);

    let request: ServerJsonrpcRequest = elicit_request.into();
    let elicit_message_str = serde_json::to_string(&request).unwrap();

    let mocks = vec![
        MockBuilder::new_sse(Method::POST, "/mcp".to_string(), INITIALIZE_RESPONSE).build(),
        MockBuilder::new_breakable_sse(
            Method::GET,
            "/mcp".to_string(),
            SseEvent {
                data: Some(elicit_message_str.into()),
                event: Some("message".to_string()),
                id: None,
            },
            Duration::from_millis(800),
            2,
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
    let (client, client_message_history) = create_client(&mcp_url, Some(headers)).await;

    client.clone().start().await.unwrap();

    assert!(client.is_initialized());

    tokio::time::sleep(Duration::from_secs(2)).await;

    handle.print().await;
    let h = client_message_history.read().await;

    let first_message: MessageFromServer = h[0].clone();
    let MessageFromServer::RequestFromServer(
        rust_mcp_sdk::schema::RequestFromServer::ElicitRequest(elicit_request),
    ) = first_message
    else {
        panic!("Expected a ElicitRequest");
    };
    assert_eq!(elicit_request.message(), "Please enter your info");

    let message_history = handle.get_history().await;

    let message_count = message_history.len();
    let entry = message_history[message_count - 2].clone();
    let result: ClientJsonrpcResponse = serde_json::from_str(&entry.0.body).unwrap();

    let ResultFromClient::CreateTaskResult(message) = result.result.clone() else {
        panic!("Expected a CreateTaskResult")
    };
    assert_eq!(message.task.status, TaskStatus::Working);
    // last message
    let entry = message_history[message_count - 1].clone();
    let notification: ClientJsonrpcNotification = serde_json::from_str(&entry.0.body).unwrap();

    let ClientJsonrpcNotification::TaskStatusNotification(notification) = notification else {
        panic!("Expected a TaskStatusNotification")
    };
    assert_eq!(notification.params.status, TaskStatus::Completed);
}
