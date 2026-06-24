//! `ClientHandler` implementation used by every scenario.
//!
//! The conformance suite exercises both **elicitation** (server asks the
//! client for structured input) and **sampling** (server asks the client
//! to run an LLM). For testing purposes we accept every elicitation form
//! by returning the schema's `default` values where present, and echo back
//! a canned text message for every sampling request. Real applications
//! should plug in their own UI / LLM here.

use async_trait::async_trait;
use rust_mcp_sdk::mcp_client::ClientHandler;
use rust_mcp_sdk::schema::{
    CreateMessageContent, CreateMessageRequestParams, CreateMessageResult, ElicitRequestParams,
    ElicitResult, ElicitResultAction, ElicitResultContent, ElicitResultContentPrimitive,
    PrimitiveSchemaDefinition, Role, RpcError, TextContent,
};
use rust_mcp_sdk::McpClient;
use std::collections::BTreeMap;

/// Default `ClientHandler` for the conformance test binary.
pub struct ConformanceClientHandler;

#[async_trait]
impl ClientHandler for ConformanceClientHandler {
    async fn handle_elicit_request(
        &self,
        params: ElicitRequestParams,
        _runtime: &dyn McpClient,
    ) -> Result<ElicitResult, RpcError> {
        match &params {
            ElicitRequestParams::FormParams(form_params) => {
                let mut content: BTreeMap<String, ElicitResultContent> = BTreeMap::new();
                for (key, schema) in &form_params.requested_schema.properties {
                    if let Some(default) = extract_default(schema) {
                        content.insert(key.clone(), default);
                    }
                }
                Ok(ElicitResult {
                    action: ElicitResultAction::Accept,
                    content: Some(content),
                    meta: None,
                })
            }
            _ => Ok(ElicitResult {
                action: ElicitResultAction::Accept,
                content: None,
                meta: None,
            }),
        }
    }

    async fn handle_create_message_request(
        &self,
        _params: CreateMessageRequestParams,
        _runtime: &dyn McpClient,
    ) -> Result<CreateMessageResult, RpcError> {
        Ok(CreateMessageResult {
            model: "echo".into(),
            role: Role::Assistant,
            content: CreateMessageContent::TextContent(TextContent::new(
                "Echo: sample received".into(),
                None,
                None,
            )),
            meta: None,
            stop_reason: None,
        })
    }
}

/// Extract the `default` value of a primitive elicitation schema, if any.
///
/// Handles the four primitive shapes commonly seen in the conformance
/// suite (string, number/integer, boolean, single-select enum). Other
/// shapes return `None`, so the server gets an empty content map for them
/// rather than a fabricated value.
fn extract_default(schema: &PrimitiveSchemaDefinition) -> Option<ElicitResultContent> {
    use ElicitResultContentPrimitive::*;
    match schema {
        PrimitiveSchemaDefinition::StringSchema(s) => s
            .default
            .as_ref()
            .map(|d| ElicitResultContent::Primitive(String(d.clone()))),
        PrimitiveSchemaDefinition::NumberSchema(n) => n
            .default
            .map(|d| ElicitResultContent::Primitive(Integer(d as i64))),
        PrimitiveSchemaDefinition::BooleanSchema(b) => b
            .default
            .map(|d| ElicitResultContent::Primitive(Boolean(d))),
        PrimitiveSchemaDefinition::UntitledSingleSelectEnumSchema(e) => e
            .default
            .as_ref()
            .map(|d| ElicitResultContent::Primitive(String(d.clone()))),
        _ => None,
    }
}
