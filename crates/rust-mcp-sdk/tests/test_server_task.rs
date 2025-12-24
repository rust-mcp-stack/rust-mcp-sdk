#[path = "common/common.rs"]
pub mod common;
mod test_streamable_http_server;

use crate::common::{
    init_tracing, read_sse_event, sample_tools::TaskAugmentedTool, send_post_request,
    task_runner::TaskJobInfo, ONE_MILLISECOND,
};
use hyper::StatusCode;
use rust_mcp_schema::{
    schema_utils::{ClientJsonrpcRequest, RequestFromClient},
    CallToolResult, RequestId, TaskMetadata, TaskStatus,
};
use rust_mcp_sdk::schema::{ResultFromServer, ServerJsonrpcNotification, ServerJsonrpcResponse};
use serde_json::json;
use std::{panic, time::Duration};
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
