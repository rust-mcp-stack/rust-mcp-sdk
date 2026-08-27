//! MRTR (mid-request turn-around, SEP-2322) fixture tools for the
//! conformance server.
//!
//! Each tool returns an `InputRequiredResult` (`resultType: "input_required"`)
//! when called without `inputResponses`, and a complete result once the
//! client retries with the resolved inputs — exercising the ephemeral
//! InputRequiredResult flow end to end. `requestState` values are
//! integrity-protected with the SDK's HMAC `RequestStateCodec`, and the
//! tampered-state tool demonstrates rejection of modified state with a
//! JSON-RPC error.

use rust_mcp_sdk::tool_box;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use rust_mcp_macros::JsonSchema;
use rust_mcp_sdk::macros::mcp_tool;
use rust_mcp_sdk::mcp_server::RequestStateCodec;
use rust_mcp_sdk::schema::{
    BooleanSchema, CallToolRequestParams, CallToolResult, ContentBlock, CreateMessageRequest,
    CreateMessageRequestParams, ElicitRequest, ElicitRequestFormParams,
    ElicitRequestFormParamsRequestedSchema, GetPromptResult, InputRequest, InputRequests,
    ListRootsRequest, PrimitiveSchemaDefinition, Prompt, PromptMessage, Role, RpcError,
    SamplingMessage, SamplingMessageContent, ServerResult, StringSchema, TextContent, Tool,
};

/// HMAC key for requestState integrity. A test fixture uses a fixed key; a
/// production deployment would load a managed secret.
static STATE_CODEC: LazyLock<RequestStateCodec> =
    LazyLock::new(|| RequestStateCodec::with_key([7u8; 32]));

/// Tool/prompt identifiers required by the SEP-2322 conformance scenarios.
pub mod names {
    pub const ELICITATION: &str = "test_input_required_result_elicitation";
    pub const SAMPLING: &str = "test_input_required_result_sampling";
    pub const LIST_ROOTS: &str = "test_input_required_result_list_roots";
    pub const REQUEST_STATE: &str = "test_input_required_result_request_state";
    pub const MULTIPLE_INPUTS: &str = "test_input_required_result_multiple_inputs";
    pub const MULTI_ROUND: &str = "test_input_required_result_multi_round";
    pub const TAMPERED_STATE: &str = "test_input_required_result_tampered_state";
    pub const CAPABILITIES: &str = "test_input_required_result_capabilities";
    pub const PROMPT: &str = "test_input_required_result_prompt";
}

// ── MRTR tool definitions ──────────────────────────────────────────

#[mcp_tool(
    name = "test_input_required_result_elicitation",
    description = "MRTR fixture: single elicitation input request."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultElicitation {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

#[mcp_tool(
    name = "test_input_required_result_sampling",
    description = "MRTR fixture: single sampling input request."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultSampling {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

#[mcp_tool(
    name = "test_input_required_result_list_roots",
    description = "MRTR fixture: single roots/list input request."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultListRoots {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

#[mcp_tool(
    name = "test_input_required_result_request_state",
    description = "MRTR fixture: requestState round-trip."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultRequestState {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

#[mcp_tool(
    name = "test_input_required_result_multiple_inputs",
    description = "MRTR fixture: multiple input requests."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultMultipleInputs {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

#[mcp_tool(
    name = "test_input_required_result_multi_round",
    description = "MRTR fixture: multi-round with evolving requestState."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultMultiRound {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

#[mcp_tool(
    name = "test_input_required_result_tampered_state",
    description = "MRTR fixture: integrity-protected requestState."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultTamperedState {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

#[mcp_tool(
    name = "test_input_required_result_capabilities",
    description = "MRTR fixture: only request inputs the client supports."
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct TestInputRequiredResultCapabilities {
    #[serde(default, skip_serializing)]
    _dummy: Option<()>,
}

tool_box!(
    MrtrTools,
    [
        TestInputRequiredResultElicitation,
        TestInputRequiredResultSampling,
        TestInputRequiredResultListRoots,
        TestInputRequiredResultRequestState,
        TestInputRequiredResultMultipleInputs,
        TestInputRequiredResultMultiRound,
        TestInputRequiredResultTamperedState,
        TestInputRequiredResultCapabilities,
    ]
);

/// The MRTR tool definitions from the `MrtrTools` tool-box, appended to
/// `tools/list` in the conformance handler.
pub fn tools() -> Vec<Tool> {
    MrtrTools::tools()
}

/// The MRTR prompt definition, appended to `prompts/list`.
pub fn prompt() -> Prompt {
    Prompt {
        name: names::PROMPT.into(),
        description: Some("MRTR fixture: prompt that requires elicitation input.".into()),
        arguments: vec![],
        icons: vec![],
        meta: None,
        title: None,
    }
}

// ── helpers ────────────────────────────────────────────────────────

/// Build an `ElicitRequest` input request with a single required
/// string or boolean property, using typed SDK constructors.
fn elicit_request(message: &str, property: &str, property_type: &str) -> InputRequest {
    let primitive: PrimitiveSchemaDefinition = match property_type {
        "string" => PrimitiveSchemaDefinition::StringSchema(StringSchema::new(
            None, None, None, None, None, None,
        )),
        "boolean" => PrimitiveSchemaDefinition::BooleanSchema(BooleanSchema::new(None, None, None)),
        other => panic!("unsupported property type '{other}' for elicit_request"),
    };

    let mut properties = BTreeMap::new();
    properties.insert(property.to_string(), primitive);

    let schema =
        ElicitRequestFormParamsRequestedSchema::new(properties, vec![property.to_string()], None);

    InputRequest::ElicitRequest(ElicitRequest::new(
        ElicitRequestFormParams::new(message.to_string(), schema).into(),
    ))
}

/// Build a `sampling/createMessage` input request with a single user text message.
pub(crate) fn sampling_request(text: &str, max_tokens: i64) -> InputRequest {
    let message = SamplingMessage {
        role: Role::User,
        content: SamplingMessageContent::TextContent(TextContent::new(
            text.to_string(),
            None,
            None,
        )),
        meta: None,
    };
    let params = CreateMessageRequestParams {
        max_tokens,
        messages: vec![message],
        include_context: None,
        metadata: None,
        model_preferences: None,
        stop_sequences: Vec::new(),
        system_prompt: None,
        temperature: None,
        tool_choice: None,
        tools: Vec::new(),
    };
    InputRequest::CreateMessageRequest(CreateMessageRequest::new(params))
}

fn list_roots_request() -> InputRequest {
    InputRequest::ListRootsRequest(ListRootsRequest::new(None))
}

pub(crate) fn input_requests(pairs: Vec<(&str, InputRequest)>) -> InputRequests {
    let mut map = BTreeMap::new();
    for (key, request) in pairs {
        map.insert(key.to_string(), request);
    }
    InputRequests(map)
}

fn input_required(requests: InputRequests, request_state: Option<String>) -> ServerResult {
    use rust_mcp_sdk::schema::InputRequiredResult;
    ServerResult::InputRequiredResult(InputRequiredResult {
        input_requests: Some(requests),
        meta: None,
        request_state,
        result_type: "input_required".to_string(),
    })
}

fn complete_text(text: impl Into<String>) -> ServerResult {
    ServerResult::CallToolResult(CallToolResult::text_content(vec![TextContent::new(
        text.into(),
        None,
        None,
    )]))
}

/// Extract a string field from the ElicitResult `content` for `key` in
/// `inputResponses` (e.g. `content.name` for the `user_name` response).
fn elicited_string(params: &CallToolRequestParams, key: &str, field: &str) -> Option<String> {
    let responses = params.input_responses.as_ref()?;
    let response = responses.0.get(key)?;
    let rust_mcp_sdk::schema::InputResponse::ElicitResult(elicit) = response else {
        return None;
    };
    let content = elicit.content.as_ref()?;
    match content.get(field)? {
        rust_mcp_sdk::schema::ElicitResultContent::Primitive(
            rust_mcp_sdk::schema::ElicitResultContentPrimitive::String(s),
        ) => Some(s.clone()),
        rust_mcp_sdk::schema::ElicitResultContent::Primitive(
            rust_mcp_sdk::schema::ElicitResultContentPrimitive::Integer(i),
        ) => Some(i.to_string()),
        rust_mcp_sdk::schema::ElicitResultContent::Primitive(
            rust_mcp_sdk::schema::ElicitResultContentPrimitive::Boolean(b),
        ) => Some(b.to_string()),
        rust_mcp_sdk::schema::ElicitResultContent::StringArray(arr) => Some(arr.join(", ")),
    }
}

// ── MRTR tool dispatch ─────────────────────────────────────────────

/// Build an `InputRequests` with a single elicitation request — a public
/// helper also used by the `test_streaming_elicitation` fixture in the
/// conformance handler.
pub fn elicit_input_requests(
    key: &str,
    message: &str,
    property: &str,
    property_type: &str,
) -> InputRequests {
    input_requests(vec![(
        key,
        elicit_request(message, property, property_type),
    )])
}

/// Dispatch an MRTR fixture tool call. Returns `None` when `params.name` is
/// not an MRTR fixture tool (the caller falls back to the normal dispatch).
pub fn handle_tool_call(params: &CallToolRequestParams) -> Option<Result<ServerResult, RpcError>> {
    let result = match params.name.as_str() {
        names::ELICITATION => handle_elicitation(params),
        names::SAMPLING => handle_sampling(params),
        names::LIST_ROOTS => handle_list_roots(params),
        names::REQUEST_STATE => handle_request_state(params),
        names::MULTIPLE_INPUTS => handle_multiple_inputs(params),
        names::MULTI_ROUND => handle_multi_round(params),
        names::TAMPERED_STATE => handle_tampered_state(params),
        names::CAPABILITIES => handle_capabilities(params),
        _ => return None,
    };
    Some(result)
}

fn handle_elicitation(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    if params.input_responses.is_none() {
        return Ok(input_required(
            input_requests(vec![(
                "user_name",
                elicit_request("What is your name?", "name", "string"),
            )]),
            None,
        ));
    }
    let name = elicited_string(params, "user_name", "name").unwrap_or_else(|| "friend".to_string());
    Ok(complete_text(format!("Hello, {name}!")))
}

fn handle_sampling(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    if params.input_responses.is_none() {
        return Ok(input_required(
            input_requests(vec![(
                "capital_question",
                sampling_request("What is the capital of France?", 100),
            )]),
            None,
        ));
    }
    let text = params
        .input_responses
        .as_ref()
        .and_then(|r| r.0.get("capital_question"))
        .and_then(|resp| match resp {
            rust_mcp_sdk::schema::InputResponse::CreateMessageResult(result) => {
                Some(match &result.content {
                    rust_mcp_sdk::schema::CreateMessageContent::TextContent(t) => t.text.clone(),
                    other => format!("{other:?}"),
                })
            }
            _ => None,
        })
        .unwrap_or_else(|| "(no sampling response)".to_string());
    Ok(complete_text(format!("Sampling answer: {text}")))
}

fn handle_list_roots(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    if params.input_responses.is_none() {
        return Ok(input_required(
            input_requests(vec![("client_roots", list_roots_request())]),
            None,
        ));
    }
    let roots = params
        .input_responses
        .as_ref()
        .and_then(|r| r.0.get("client_roots"))
        .and_then(|resp| match resp {
            rust_mcp_sdk::schema::InputResponse::ListRootsResult(result) => {
                Some(result.roots.clone())
            }
            _ => None,
        })
        .unwrap_or_default();
    let summary = roots
        .iter()
        .map(|r| r.uri.clone())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(complete_text(format!("Client roots: {summary}")))
}

fn handle_request_state(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    if params.input_responses.is_none() {
        let state = STATE_CODEC.encode(b"request-state-round-1");
        return Ok(input_required(
            input_requests(vec![(
                "confirm",
                elicit_request("Please confirm", "ok", "boolean"),
            )]),
            Some(state),
        ));
    }
    let state = params.request_state.as_deref().ok_or_else(|| {
        RpcError::invalid_params().with_message("missing requestState on retry".to_string())
    })?;
    if STATE_CODEC.decode(state).is_none() {
        return Err(RpcError::invalid_params()
            .with_message("requestState failed integrity verification".to_string()));
    }
    Ok(complete_text("state-ok"))
}

fn handle_multiple_inputs(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    if params.input_responses.is_none() {
        let state = STATE_CODEC.encode(b"multiple-inputs");
        return Ok(input_required(
            input_requests(vec![
                (
                    "user_name",
                    elicit_request("What is your name?", "name", "string"),
                ),
                ("greeting", sampling_request("Generate a greeting", 50)),
                ("client_roots", list_roots_request()),
            ]),
            Some(state),
        ));
    }
    if let Some(state) = params.request_state.as_deref() {
        if STATE_CODEC.decode(state).is_none() {
            return Err(RpcError::invalid_params()
                .with_message("requestState failed integrity verification".to_string()));
        }
    }
    Ok(complete_text("multiple-inputs-ok"))
}

fn handle_multi_round(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    let responses = params.input_responses.as_ref();
    let state = params.request_state.as_deref();

    match (responses, state) {
        (None, _) => Ok(input_required(
            input_requests(vec![(
                "step1",
                elicit_request("Step 1: What is your name?", "name", "string"),
            )]),
            Some(STATE_CODEC.encode(b"round-1")),
        )),
        (Some(r), Some(s)) if STATE_CODEC.decode(s).as_deref() == Some(b"round-1") => {
            if !r.0.contains_key("step1") {
                return Ok(input_required(
                    input_requests(vec![(
                        "step1",
                        elicit_request("Step 1: What is your name?", "name", "string"),
                    )]),
                    Some(STATE_CODEC.encode(b"round-1")),
                ));
            }
            Ok(input_required(
                input_requests(vec![(
                    "step2",
                    elicit_request("Step 2: What is your favorite color?", "color", "string"),
                )]),
                Some(STATE_CODEC.encode(b"round-2")),
            ))
        }
        (Some(r), Some(s)) if STATE_CODEC.decode(s).as_deref() == Some(b"round-2") => {
            if !r.0.contains_key("step2") {
                return Ok(input_required(
                    input_requests(vec![(
                        "step2",
                        elicit_request("Step 2: What is your favorite color?", "color", "string"),
                    )]),
                    Some(STATE_CODEC.encode(b"round-2")),
                ));
            }
            Ok(complete_text("multi-round-complete"))
        }
        (Some(_), Some(_)) => Err(RpcError::invalid_params()
            .with_message("requestState failed integrity verification".to_string())),
        (Some(_), None) => {
            Err(RpcError::invalid_params()
                .with_message("missing requestState on retry".to_string()))
        }
    }
}

fn handle_tampered_state(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    if params.input_responses.is_none() {
        let state = STATE_CODEC.encode(b"tamper-evident-state");
        return Ok(input_required(
            input_requests(vec![(
                "confirm",
                elicit_request("Please confirm", "ok", "boolean"),
            )]),
            Some(state),
        ));
    }
    let state = params.request_state.as_deref().ok_or_else(|| {
        RpcError::invalid_params().with_message("missing requestState on retry".to_string())
    })?;
    if STATE_CODEC.decode(state).is_none() {
        return Err(RpcError::invalid_params()
            .with_message("requestState failed integrity verification".to_string()));
    }
    Ok(complete_text("tampered-state-ok"))
}

fn handle_capabilities(params: &CallToolRequestParams) -> Result<ServerResult, RpcError> {
    if params.input_responses.is_none() {
        let caps = &params.meta.client_capabilities;
        let mut requests: Vec<(&str, InputRequest)> = Vec::new();
        if caps.sampling.is_some() {
            requests.push((
                "capital_question",
                sampling_request("What is the capital of France?", 100),
            ));
        }
        if caps.elicitation.is_some() {
            requests.push((
                "user_name",
                elicit_request("What is your name?", "name", "string"),
            ));
        }
        if caps.roots.is_some() {
            requests.push(("client_roots", list_roots_request()));
        }
        return Ok(input_required(input_requests(requests), None));
    }
    Ok(complete_text("capabilities-ok"))
}

/// Dispatch the MRTR fixture prompt (`prompts/get`).
/// Returns `None` when `name` is not the MRTR fixture prompt.
pub fn handle_prompt_get(
    name: &str,
    params: &rust_mcp_sdk::schema::GetPromptRequestParams,
) -> Option<Result<ServerResult, RpcError>> {
    if name != names::PROMPT {
        return None;
    }
    if params.input_responses.is_none() {
        return Some(Ok(input_required(
            input_requests(vec![(
                "user_context",
                elicit_request("What context should the prompt use?", "context", "string"),
            )]),
            None,
        )));
    }
    let context = params
        .input_responses
        .as_ref()
        .and_then(|r| r.0.get("user_context"))
        .and_then(|resp| match resp {
            rust_mcp_sdk::schema::InputResponse::ElicitResult(elicit) => elicit
                .content
                .as_ref()
                .and_then(|c| match c.get("context")? {
                    rust_mcp_sdk::schema::ElicitResultContent::Primitive(
                        rust_mcp_sdk::schema::ElicitResultContentPrimitive::String(s),
                    ) => Some(s.clone()),
                    _ => None,
                }),
            _ => None,
        })
        .unwrap_or_else(|| "general".to_string());

    let result = GetPromptResult {
        result_type: "complete".to_string(),
        description: Some("MRTR fixture prompt".into()),
        messages: vec![PromptMessage {
            role: Role::User,
            content: ContentBlock::text_content(format!("Prompt using context: {context}")),
        }],
        meta: None,
    };
    Some(Ok(ServerResult::GetPromptResult(result)))
}
