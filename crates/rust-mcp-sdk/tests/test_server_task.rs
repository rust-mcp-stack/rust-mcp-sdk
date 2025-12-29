#[path = "common/common.rs"]
pub mod common;
mod test_streamable_http_server;

use crate::common::{
    init_tracing, read_sse_event, sample_tools::TaskAugmentedTool, send_post_request,
    task_runner::TaskJobInfo, ONE_MILLISECOND,
};
use hyper::StatusCode;
use rust_mcp_macros::{mcp_elicit, JsonSchema};
use rust_mcp_schema::{
    schema_utils::{ClientJsonrpcRequest, RequestFromClient},
    CallToolResult, CreateTaskResult, ElicitRequestParams, ElicitResult, ElicitResultAction,
    ElicitResultContent, ElicitResultContentPrimitive, GetTaskResult, RequestId, Task,
    TaskMetadata, TaskStatus,
};
use rust_mcp_sdk::schema::{
    ClientJsonrpcResponse, ResultFromServer, ServerJsonrpcNotification, ServerJsonrpcResponse,
};
use serde_json::json;
use std::{collections::HashMap, panic, sync::Arc, time::Duration};
use test_streamable_http_server::*;

#[tokio::test]
async fn test_server_task_normal() {
    init_tracing();
    let (server, session_id) = initialize_server(None, None).await.unwrap();

    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let expected_result: ResultFromServer =
        CallToolResult::text_content(vec!["task-completed".into()]).into();

    let task_info = TaskJobInfo {
        finish_in_ms: 1200,
        status_interval_ms: 250,
        task_final_status: TaskStatus::Completed.to_string(),
        task_result: Some(serde_json::to_string(&expected_result).unwrap()),

        meta: Some(
            json!({"task_meta":"meta_value"})
                .as_object()
                .cloned()
                .unwrap(),
        ),
    };
    let v = serde_json::to_value(task_info)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    let aruments = TaskAugmentedTool::request_params()
        .with_arguments(v)
        .with_task(TaskMetadata { ttl: None });

    let json_rpc_message: ClientJsonrpcRequest = ClientJsonrpcRequest::new(
        RequestId::Integer(1),
        RequestFromClient::CallToolRequest(aruments).into(),
    );

    let resp = send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    let messages = read_sse_event(resp, 1).await.unwrap();
    let result_message: ServerJsonrpcResponse = serde_json::from_str(&messages[0].2).unwrap();

    let ResultFromServer::CreateTaskResult(create_task_result) = result_message.result else {
        panic!("Expected a CreateTaskResult!");
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    let messages = read_sse_event(response, 1).await.unwrap();

    let message: ServerJsonrpcNotification = serde_json::from_str(&messages[0].2).unwrap();

    let ServerJsonrpcNotification::TaskStatusNotification(notification) = message else {
        panic!("Expected a TaskStatusNotification")
    };
    assert_eq!(notification.params.status, TaskStatus::Completed);
    assert!(notification.params.status_message.is_none());
    assert_eq!(
        notification.params.meta,
        Some(
            json!( {"task_meta": "meta_value"})
                .as_object()
                .cloned()
                .unwrap()
        )
    );

    let store = server.hyper_runtime.task_store().unwrap().clone();
    let task_result = store
        .get_task_result(&create_task_result.task.task_id, Some(session_id))
        .await
        .unwrap();

    let ResultFromServer::CallToolResult(task_result) = task_result else {
        panic!("expected a CallToolResult!");
    };
    assert_eq!(task_result.content.len(), 1);
    let text_content = task_result.content[0].as_text_content().unwrap();
    assert_eq!(text_content.text, "task-completed");

    server.hyper_runtime.graceful_shutdown(ONE_MILLISECOND);
    server.hyper_runtime.await_server().await.unwrap()
}

#[tokio::test]
async fn test_server_task_wait_for_result() {
    #[mcp_elicit(message = "Please enter your info", mode = form)]
    #[derive(JsonSchema)]
    pub struct UserEmail {
        #[json_schema(title = "Email", format = "email")]
        pub email: Option<String>,
    }

    init_tracing();
    let (server, session_id) = initialize_server(None, None).await.unwrap();

    let response = get_standalone_stream(&server.streamable_url, &session_id, None).await;
    assert_eq!(response.status(), StatusCode::OK);

    let elicit_params: ElicitRequestParams =
        UserEmail::elicit_request_params().with_task(TaskMetadata { ttl: Some(10000) });

    let hyper_server = Arc::new(server.hyper_runtime);
    let hyper_server_clone = hyper_server.clone();

    let session_id_clone = session_id.clone();
    tokio::spawn(async move {
        let task_result = hyper_server_clone
            .request_elicitation_task(&session_id_clone, elicit_params)
            .await
            .unwrap();

        let task_store = hyper_server_clone.client_task_store().unwrap();
        let task_result = task_store
            .wait_for_task_result(&task_result.task.task_id, Some(session_id_clone))
            .await
            .unwrap();

        assert_eq!(task_result.0, TaskStatus::Completed);
        let elicit_result: ElicitResult = task_result.1.unwrap().try_into().unwrap();
        assert_eq!(elicit_result.action, ElicitResultAction::Accept);
        let email_value = elicit_result
            .content
            .as_ref()
            .unwrap()
            .get("email")
            .unwrap();

        let ElicitResultContent::Primitive(ElicitResultContentPrimitive::String(email)) =
            email_value
        else {
            panic!("invalid elicit result content type");
        };
        assert_eq!(email, "email@example.com");
    });
    let res = CreateTaskResult {
        meta: None,
        task: Task {
            created_at: "".to_string(),
            last_updated_at: "".to_string(),
            poll_interval: Some(500),
            status: TaskStatus::Working,
            status_message: None,
            task_id: "tskAAAAAAAAAAA".to_string(),
            ttl: Some(60_000),
        },
    };

    // send taskcreate result
    let json_rpc_message: ClientJsonrpcResponse =
        ClientJsonrpcResponse::new(RequestId::Integer(0), res.into());
    send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    tokio::time::sleep(Duration::from_millis(2200)).await;

    let get_task_result = GetTaskResult {
        created_at: "".to_string(),
        last_updated_at: "".to_string(),
        meta: None,
        poll_interval: Some(250),
        status: TaskStatus::Completed,
        status_message: None,
        task_id: "tskAAAAAAAAAAA".to_string(),
        ttl: 60_000,
        extra: None,
    };

    let json_rpc_message: ClientJsonrpcResponse =
        ClientJsonrpcResponse::new(RequestId::Integer(1), get_task_result.into());
    send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut data: HashMap<String, ElicitResultContent> = HashMap::new();
    data.insert("email".into(), "email@example.com".into());
    let elicit_result: ElicitResult = ElicitResult {
        action: rust_mcp_schema::ElicitResultAction::Accept,
        content: Some(data),
        meta: None,
    };

    let json_rpc_message: ClientJsonrpcResponse =
        ClientJsonrpcResponse::new(RequestId::Integer(2), elicit_result.into());
    send_post_request(
        &server.streamable_url,
        &serde_json::to_string(&json_rpc_message).unwrap(),
        Some(&session_id),
        None,
    )
    .await
    .expect("Request failed");

    hyper_server.graceful_shutdown(ONE_MILLISECOND);
}
