//! `http-standard-headers` scenario (SEP-2243) — exercise the standard
//! `Mcp-Method` / `Mcp-Name` request headers on the streamable-HTTP wire.
//!
//! The SDK runtime stamps `Mcp-Method` on every request and `Mcp-Name` on
//! requests that target a named object (`tools/call` → `params.name`,
//! `resources/read` → `params.uri`, `prompts/get` → `params.name`). This
//! scenario drives one of each request kind so the harness can observe the
//! headers; the assertions live on the harness side.

use rust_mcp_sdk::schema::{
    CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams, RequestMetaObject,
};

use crate::client::transport;

pub async fn run(server_url: &str) {
    let client = transport::connect(server_url)
        .await
        .expect("Failed to connect");

    // tools/list + tools/call (Mcp-Name mirrors params.name)
    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");
    if let Some(tool) = tools.tools.first() {
        let _ = client
            .request_tool_call(CallToolRequestParams {
                name: tool.name.clone(),
                arguments: Some(serde_json::Map::new()),
                meta: RequestMetaObject::default(),
                input_responses: None,
                request_state: None,
            })
            .await;
    }

    // resources/list + resources/read (Mcp-Name mirrors params.uri)
    let resources = client
        .request_resource_list(None)
        .await
        .expect("Failed to list resources");
    if let Some(resource) = resources.resources.first() {
        let _ = client
            .request_resource_read(ReadResourceRequestParams {
                uri: resource.uri.clone(),
                meta: RequestMetaObject::default(),
                input_responses: None,
                request_state: None,
            })
            .await;
    }

    // prompts/list + prompts/get (Mcp-Name mirrors params.name)
    let prompts = client
        .request_prompt_list(None)
        .await
        .expect("Failed to list prompts");
    if let Some(prompt) = prompts.prompts.first() {
        let _ = client
            .request_prompt(GetPromptRequestParams {
                name: prompt.name.clone(),
                arguments: None,
                meta: RequestMetaObject::default(),
                input_responses: None,
                request_state: None,
            })
            .await;
    }

    client.shut_down().await.ok();
}
