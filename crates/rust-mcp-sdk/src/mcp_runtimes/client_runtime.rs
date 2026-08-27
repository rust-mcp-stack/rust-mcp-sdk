pub mod mcp_client_runtime;
pub mod mcp_client_runtime_core;
pub(crate) mod response_cache;
use crate::error::{McpSdkError, SdkResult};
#[cfg(feature = "streamable-http")]
use crate::id_generator::FastIdGenerator;
use crate::mcp_traits::ClientDetails;
use crate::mcp_traits::{McpClient, McpClientHandler};
use crate::McpObserver;
use crate::{
    mcp_traits::{RequestIdGen, RequestIdGenNumeric},
    schema::{
        schema_utils::{
            ClientMessage, ClientMessages, FromMessage, MessageFromClient, RequestFromClient,
            ResultFromClient, ServerJsonrpcRequest, ServerMessage, ServerMessages,
        },
        CallToolRequestParams, CallToolResult, GetPromptRequestParams, GetPromptResult,
        InputRequests, InputResponses, PaginatedRequestParams, Prompt, ReadResourceRequestParams,
        ReadResourceResult, RequestId, Resource, ResourceTemplate, RpcError, ServerResult, Tool,
    },
};
use async_trait::async_trait;
use futures::StreamExt;
#[cfg(feature = "streamable-http")]
use rust_mcp_transport::StreamId;
#[cfg(feature = "streamable-http")]
use rust_mcp_transport::{ClientStreamableTransport, StreamableTransportOptions};
use rust_mcp_transport::{IoStream, SessionId, TransportDispatcher};
use std::collections::HashSet;
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader, Lines};
use tokio::sync::Mutex;

use response_cache::ResponseCache;

#[cfg(feature = "streamable-http")]
pub const DEFAULT_STREAM_ID: &str = "STANDALONE-STREAM";

/// Per-round timeout for MRTR (mid-request turn-around) retries.
///
/// Each `tools/call` / `prompts/get` / `resources/read` auto-driver round is
/// bounded so a server that repeatedly returns `InputRequiredResult` without
/// completing cannot pin the client forever. Rounds are also capped
/// (10 by default).
pub(crate) const MRTR_ROUND_TIMEOUT: Duration = Duration::from_secs(30);

// Define a type alias for the TransportDispatcher trait object
type TransportDispatcherType = dyn TransportDispatcher<
    ServerMessages,
    MessageFromClient,
    ServerMessage,
    ClientMessages,
    ClientMessage,
>;
type TransportType = Arc<TransportDispatcherType>;

async fn next_process_error<R>(reader: &mut Lines<BufReader<R>>) -> std::io::Result<Option<String>>
where
    R: AsyncRead + Unpin,
{
    reader.next_line().await
}

pub struct McpClientOptions<T>
where
    T: TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    >,
{
    pub client_details: ClientDetails,
    pub transport: T,
    pub handler: Box<dyn McpClientHandler>,
    pub message_observer: Option<Arc<dyn McpObserver<ServerMessage, ClientMessage>>>,
    pub response_cache_config: Option<response_cache::ResponseCacheConfig>,
}

impl<T> McpClientOptions<T>
where
    T: TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    >,
{
    pub fn new(
        client_details: ClientDetails,
        transport: T,
        handler: Box<dyn McpClientHandler>,
    ) -> Self {
        Self {
            client_details,
            transport,
            handler,
            message_observer: None,
            response_cache_config: None,
        }
    }

    pub fn with_message_observer(
        mut self,
        observer: Arc<dyn McpObserver<ServerMessage, ClientMessage>>,
    ) -> Self {
        self.message_observer = Some(observer);
        self
    }

    pub fn with_response_cache(mut self, config: response_cache::ResponseCacheConfig) -> Self {
        self.response_cache_config = Some(config);
        self
    }
}

pub struct ClientRuntime {
    // A thread-safe map storing transport types
    transport_map: tokio::sync::RwLock<Option<TransportType>>,
    // The handler for processing MCP messages
    handler: Box<dyn McpClientHandler>,
    // Information about the client (identity + capabilities, stamped per request)
    client_details: ClientDetails,
    handlers: Mutex<Vec<tokio::task::JoinHandle<Result<(), McpSdkError>>>>,
    // Generator for unique request IDs
    request_id_gen: Box<dyn RequestIdGen>,
    // Generator for stream IDs
    #[cfg(feature = "streamable-http")]
    stream_id_gen: FastIdGenerator,
    #[cfg(feature = "streamable-http")]
    // Optional configuration for streamable transport
    transport_options: Option<StreamableTransportOptions>,
    // Flag indicating whether the client has been shut down
    is_shut_down: Mutex<bool>,
    message_observer: Option<Arc<dyn McpObserver<ServerMessage, ClientMessage>>>,
    // SEP-2243: validated `x-mcp-header` annotations per tool, used to mirror
    // tool arguments into `Mcp-Param-*` headers on `tools/call` requests.
    tool_header_registry: crate::tool_param_headers::ToolHeaderRegistry,
    // SEP-2549: response cache for list/read/discover results
    response_cache: Mutex<Option<ResponseCache>>,
}

impl ClientRuntime {
    pub(crate) fn new(
        client_details: ClientDetails,
        transport: TransportType,
        handler: Box<dyn McpClientHandler>,
        message_observer: Option<Arc<dyn McpObserver<ServerMessage, ClientMessage>>>,
    ) -> Self {
        Self {
            transport_map: tokio::sync::RwLock::new(Some(transport)),
            handler,
            client_details,
            handlers: Mutex::new(vec![]),
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
            #[cfg(feature = "streamable-http")]
            transport_options: None,
            is_shut_down: Mutex::new(false),
            #[cfg(feature = "streamable-http")]
            stream_id_gen: FastIdGenerator::new(Some("s_")),
            message_observer,
            tool_header_registry: crate::tool_param_headers::new_tool_header_registry(),
            response_cache: Mutex::new(None),
        }
    }

    #[cfg(feature = "streamable-http")]
    pub(crate) fn new_instance(
        client_details: ClientDetails,
        mut transport_options: StreamableTransportOptions,
        handler: Box<dyn McpClientHandler>,
        message_observer: Option<Arc<dyn McpObserver<ServerMessage, ClientMessage>>>,
    ) -> Self {
        // 2026-07-28 stateless protocol: every request must declare the
        // negotiated protocol version via the `mcp-protocol-version` header
        // (mirrored in `_meta`). Inject it here so transports built from
        // `StreamableTransportOptions` always send it, unless the caller
        // explicitly overrides it.
        {
            use crate::schema::ProtocolVersion;
            use rust_mcp_transport::MCP_PROTOCOL_VERSION_HEADER;
            let version = ProtocolVersion::latest().to_string();
            let headers = transport_options
                .request_options
                .custom_headers
                .get_or_insert_with(std::collections::HashMap::new);
            headers
                .entry(MCP_PROTOCOL_VERSION_HEADER.to_string())
                .or_insert(version);
        }

        // SEP-2243: install a per-request header provider that (a) emits the
        // standard `Mcp-Method` / `Mcp-Name` headers and (b) mirrors
        // `x-mcp-header`-annotated tool arguments into `Mcp-Param-*` headers
        // on `tools/call` POSTs. A caller-provided provider (if any) is chained
        // afterwards and takes precedence on header-name conflicts.
        let tool_header_registry = crate::tool_param_headers::new_tool_header_registry();
        {
            let registry = tool_header_registry.clone();
            let user_provider = transport_options
                .request_options
                .request_header_provider
                .take();
            transport_options.request_options.request_header_provider =
                Some(Arc::new(move |payload: &str| {
                    let mut headers = crate::tool_param_headers::standard_header_provider(payload);
                    if let Some(mcp_param_headers) =
                        crate::tool_param_headers::request_header_provider(payload, &registry)
                    {
                        headers
                            .get_or_insert_with(reqwest::header::HeaderMap::new)
                            .extend(mcp_param_headers);
                    }
                    if let Some(user_provider) = &user_provider {
                        if let Some(user_headers) = user_provider(payload) {
                            headers
                                .get_or_insert_with(reqwest::header::HeaderMap::new)
                                .extend(user_headers);
                        }
                    }
                    headers
                }));
        }
        Self {
            transport_map: tokio::sync::RwLock::new(None),
            handler,
            client_details,
            handlers: Mutex::new(vec![]),
            transport_options: Some(transport_options),
            is_shut_down: Mutex::new(false),
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
            #[cfg(feature = "streamable-http")]
            stream_id_gen: FastIdGenerator::new(Some("s_")),
            message_observer,
            tool_header_registry,
            response_cache: Mutex::new(None),
        }
    }

    pub(crate) async fn handle_message(
        &self,
        message: ServerMessage,
        transport: &TransportType,
    ) -> SdkResult<Option<ClientMessage>> {
        // telemetry
        if let Some(observer) = self.message_observer.as_ref() {
            observer.on_receive(&message);
        }

        let response = match message {
            ServerMessage::Request(jsonrpc_request) => {
                let request_id = jsonrpc_request.request_id().clone();
                let result = self.handler.handle_request(jsonrpc_request, self).await;

                // create a response to send back to the server
                let response: MessageFromClient = match result {
                    Ok(success_value) => success_value.into(),
                    Err(error_value) => MessageFromClient::Error(error_value),
                };

                let mcp_message = ClientMessage::from_message(response, Some(request_id))?;
                Some(mcp_message)
            }
            ServerMessage::Notification(jsonrpc_notification) => {
                // SEP-2549: invalidate cache on relevant notifications
                let notification: crate::schema::schema_utils::NotificationFromServer =
                    jsonrpc_notification.into();
                match &notification {
                    crate::schema::schema_utils::NotificationFromServer::ResourceListChangedNotification(_) => {
                        let mut lock = self.response_cache.lock().await;
                        if let Some(cache) = lock.as_mut() {
                            cache.invalidate_method("resources/list");
                        }
                    }
                    crate::schema::schema_utils::NotificationFromServer::ResourceUpdatedNotification(params) => {
                        let mut lock = self.response_cache.lock().await;
                        if let Some(cache) = lock.as_mut() {
                            cache.invalidate_method_key("resources/read", &params.uri);
                        }
                    }
                    crate::schema::schema_utils::NotificationFromServer::PromptListChangedNotification(_) => {
                        let mut lock = self.response_cache.lock().await;
                        if let Some(cache) = lock.as_mut() {
                            cache.invalidate_method("prompts/list");
                        }
                    }
                    crate::schema::schema_utils::NotificationFromServer::ToolListChangedNotification(_) => {
                        let mut lock = self.response_cache.lock().await;
                        if let Some(cache) = lock.as_mut() {
                            cache.invalidate_method("tools/list");
                        }
                    }
                    _ => {}
                }
                self.handler.handle_notification(notification, self).await?;
                None
            }
            ServerMessage::Error(jsonrpc_error) => {
                self.handler
                    .handle_error(&jsonrpc_error.error, self)
                    .await?;
                if let Some(request_id) = jsonrpc_error.id.as_ref() {
                    if let Some(tx_response) = transport.pending_request_tx(request_id).await {
                        tx_response
                            .send(ServerMessage::Error(jsonrpc_error))
                            .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                    } else {
                        tracing::warn!(
                            "Received an error response with no corresponding request: {:?}",
                            &request_id
                        );
                    }
                }
                None
            }
            ServerMessage::Response(response) => {
                if let Some(tx_response) = transport.pending_request_tx(&response.id).await {
                    tx_response
                        .send(ServerMessage::Response(response))
                        .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                } else {
                    tracing::warn!(
                        "Received a response with no corresponding request: {:?}",
                        &response.id
                    );
                }
                None
            }
        };
        Ok(response)
    }

    async fn start_standalone(self: Arc<Self>) -> SdkResult<()> {
        let self_clone = self.clone();
        let transport_map = self_clone.transport_map.read().await;
        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        let mut stream = transport.start().await?;

        let mut error_io_stream = transport.error_stream().write().await;
        let error_io_stream = error_io_stream.take();

        let self_clone = Arc::clone(&self);
        let self_clone_err = Arc::clone(&self);

        // task reading from the error stream
        let err_task = tokio::spawn(async move {
            let self_ref = &*self_clone_err;

            if let Some(IoStream::Readable(error_input)) = error_io_stream {
                let mut reader = BufReader::new(error_input).lines();
                loop {
                    match next_process_error(&mut reader).await {
                        Ok(Some(error_message)) => {
                            self_ref
                                .handler
                                .handle_process_error(error_message, self_ref)
                                .await?;
                        }
                        Ok(None) => {
                            // Transport shutdown terminates the child and closes stderr.
                            break;
                        }
                        Err(e) => {
                            tracing::error!("Error reading from std_err: {e}");
                            break;
                        }
                    }
                }
            }

            Ok::<(), McpSdkError>(())
        });

        let transport = transport.clone();

        // main task reading from mcp_message stream
        let main_task = tokio::spawn(async move {
            while let Some(mcp_messages) = stream.next().await {
                let self_ref = &*self_clone;

                match mcp_messages {
                    ServerMessages::Single(server_message) => {
                        let result = self_ref.handle_message(server_message, &transport).await;

                        match result {
                            Ok(result) => {
                                if let Some(result) = result {
                                    transport
                                        .send_message(ClientMessages::Single(result), None)
                                        .await?;
                                }
                            }
                            Err(error) => {
                                tracing::error!("Error handling message : {}", error)
                            }
                        }
                    }
                    ServerMessages::Batch(_) => {}
                }
            }
            Ok::<(), McpSdkError>(())
        });

        let mut lock = self.handlers.lock().await;
        lock.push(main_task);
        lock.push(err_task);
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn store_transport(
        &self,
        stream_id: &str,
        transport: TransportType,
    ) -> SdkResult<()> {
        let mut transport_map = self.transport_map.write().await;
        tracing::trace!("save transport for stream id : {}", stream_id);
        *transport_map = Some(transport);
        Ok(())
    }

    #[cfg(feature = "streamable-http")]
    pub(crate) async fn new_transport(
        &self,
    ) -> SdkResult<
        impl TransportDispatcher<
            ServerMessages,
            MessageFromClient,
            ServerMessage,
            ClientMessages,
            ClientMessage,
        >,
    > {
        use rust_mcp_schema::schema_utils::SdkError;

        let options = self
            .transport_options
            .as_ref()
            .ok_or(SdkError::connection_closed())?;
        let transport = ClientStreamableTransport::new(options)?;

        Ok(transport)
    }

    // --- MRTR (mid-request turn-around) ---
    async fn process_input_requests(&self, requests: InputRequests) -> SdkResult<InputResponses> {
        let mut map = std::collections::BTreeMap::new();
        for (key, input_request) in requests.0.iter() {
            let server_req = match input_request {
                rust_mcp_schema::InputRequest::CreateMessageRequest(req) => {
                    ServerJsonrpcRequest::CreateMessageRequest {
                        id: RequestId::String(key.clone()),
                        jsonrpc: rust_mcp_schema::JSONRPC_VERSION.to_string(),
                        request: req.clone(),
                    }
                }
                rust_mcp_schema::InputRequest::ListRootsRequest(req) => {
                    ServerJsonrpcRequest::ListRootsRequest {
                        id: RequestId::String(key.clone()),
                        jsonrpc: rust_mcp_schema::JSONRPC_VERSION.to_string(),
                        request: req.clone(),
                    }
                }
                rust_mcp_schema::InputRequest::ElicitRequest(req) => {
                    ServerJsonrpcRequest::ElicitRequest {
                        id: RequestId::String(key.clone()),
                        jsonrpc: rust_mcp_schema::JSONRPC_VERSION.to_string(),
                        request: req.clone(),
                    }
                }
            };
            let result = self.handler.handle_request(server_req, self).await?;
            let response = match result {
                ResultFromClient::CreateMessageResult(r) => {
                    rust_mcp_schema::InputResponse::CreateMessageResult(r)
                }
                ResultFromClient::ListRootsResult(r) => {
                    rust_mcp_schema::InputResponse::ListRootsResult(r)
                }
                ResultFromClient::ElicitResult(r) => {
                    rust_mcp_schema::InputResponse::ElicitResult(r)
                }
                _ => {
                    let e: McpSdkError = RpcError::internal_error()
                        .with_message("Unexpected MRTR input result")
                        .into();
                    return Err(e);
                }
            };
            map.insert(key.clone(), response);
        }
        Ok(InputResponses(map))
    }
    // --- Response cache (SEP-2549) ---

    pub(crate) fn init_response_cache(&self, config: response_cache::ResponseCacheConfig) {
        let mut cache = self.response_cache.blocking_lock();
        *cache = Some(ResponseCache::new(config));
    }

    /// Returns the auth principal from the current session, if available.
    async fn auth_principal(&self) -> Option<String> {
        self.session_id().await.map(|sid| sid.to_string())
    }

    // --- Auto-pagination helpers ---

    const DEFAULT_MAX_PAGES: usize = 64;

    pub async fn list_all_tools(
        &self,
        params: Option<PaginatedRequestParams>,
        max_pages: Option<usize>,
    ) -> SdkResult<Vec<Tool>> {
        let max_pages = max_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let mut all: Vec<Tool> = Vec::new();
        let mut current_params = params.unwrap_or_default();
        let mut visited: HashSet<String> = HashSet::new();

        for page in 0..max_pages {
            let result = self.request_tool_list(Some(current_params.clone())).await?;
            all.extend(result.tools);

            match result.next_cursor {
                Some(ref cursor) => {
                    if !cursor.is_empty() && !visited.insert(cursor.clone()) {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Auto-pagination: cycle detected".to_string())
                            .into();
                        return Err(e);
                    }
                    current_params.cursor = Some(cursor.clone());
                }
                None => break,
            }

            if page + 1 >= max_pages {
                let e: McpSdkError = RpcError::internal_error()
                    .with_message(format!("Auto-pagination exceeded max pages ({max_pages})"))
                    .into();
                return Err(e);
            }
        }

        Ok(all)
    }

    pub async fn list_all_prompts(
        &self,
        params: Option<PaginatedRequestParams>,
        max_pages: Option<usize>,
    ) -> SdkResult<Vec<Prompt>> {
        let max_pages = max_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let mut all: Vec<Prompt> = Vec::new();
        let mut current_params = params.unwrap_or_default();
        let mut visited: HashSet<String> = HashSet::new();

        for page in 0..max_pages {
            let result = self
                .request_prompt_list(Some(current_params.clone()))
                .await?;
            all.extend(result.prompts);

            match result.next_cursor {
                Some(ref cursor) => {
                    if !cursor.is_empty() && !visited.insert(cursor.clone()) {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Auto-pagination: cycle detected".to_string())
                            .into();
                        return Err(e);
                    }
                    current_params.cursor = Some(cursor.clone());
                }
                None => break,
            }

            if page + 1 >= max_pages {
                let e: McpSdkError = RpcError::internal_error()
                    .with_message(format!("Auto-pagination exceeded max pages ({max_pages})"))
                    .into();
                return Err(e);
            }
        }

        Ok(all)
    }

    pub async fn list_all_resources(
        &self,
        params: Option<PaginatedRequestParams>,
        max_pages: Option<usize>,
    ) -> SdkResult<Vec<Resource>> {
        let max_pages = max_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let mut all: Vec<Resource> = Vec::new();
        let mut current_params = params.unwrap_or_default();
        let mut visited: HashSet<String> = HashSet::new();

        for page in 0..max_pages {
            let result = self
                .request_resource_list(Some(current_params.clone()))
                .await?;
            all.extend(result.resources);

            match result.next_cursor {
                Some(ref cursor) => {
                    if !cursor.is_empty() && !visited.insert(cursor.clone()) {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Auto-pagination: cycle detected".to_string())
                            .into();
                        return Err(e);
                    }
                    current_params.cursor = Some(cursor.clone());
                }
                None => break,
            }

            if page + 1 >= max_pages {
                let e: McpSdkError = RpcError::internal_error()
                    .with_message(format!("Auto-pagination exceeded max pages ({max_pages})"))
                    .into();
                return Err(e);
            }
        }

        Ok(all)
    }

    pub async fn list_all_resource_templates(
        &self,
        params: Option<PaginatedRequestParams>,
        max_pages: Option<usize>,
    ) -> SdkResult<Vec<ResourceTemplate>> {
        let max_pages = max_pages.unwrap_or(Self::DEFAULT_MAX_PAGES);
        let mut all: Vec<ResourceTemplate> = Vec::new();
        let mut current_params = params.unwrap_or_default();
        let mut visited: HashSet<String> = HashSet::new();

        for page in 0..max_pages {
            let result = self
                .request_resource_template_list(Some(current_params.clone()))
                .await?;
            all.extend(result.resource_templates);

            match result.next_cursor {
                Some(ref cursor) => {
                    if !cursor.is_empty() && !visited.insert(cursor.clone()) {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Auto-pagination: cycle detected".to_string())
                            .into();
                        return Err(e);
                    }
                    current_params.cursor = Some(cursor.clone());
                }
                None => break,
            }

            if page + 1 >= max_pages {
                let e: McpSdkError = RpcError::internal_error()
                    .with_message(format!("Auto-pagination exceeded max pages ({max_pages})"))
                    .into();
                return Err(e);
            }
        }

        Ok(all)
    }

    /// MRTR-aware `tools/call`. Retries up to 10 rounds.
    pub async fn call_tool(&self, params: CallToolRequestParams) -> SdkResult<CallToolResult> {
        const MAX_ROUNDS: usize = 10;
        let span = tracing::info_span!("mcp.mrtr", method = "tools/call", tool = %params.name);
        let _enter = span.enter();
        let mut current_params = params;
        for _round in 0..MAX_ROUNDS {
            let response = self
                .request(
                    crate::schema::schema_utils::RequestFromClient::CallToolRequest(
                        current_params.clone(),
                    ),
                    Some(MRTR_ROUND_TIMEOUT),
                )
                .await?;
            match response {
                ServerResult::CallToolResult(r) => {
                    if r.result_type != "complete" {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Invalid resultType for CallToolResult")
                            .into();
                        return Err(e);
                    }
                    return Ok(r);
                }
                ServerResult::InputRequiredResult(iro) => {
                    if iro.result_type != "input_required" {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Invalid resultType for InputRequiredResult")
                            .into();
                        return Err(e);
                    }
                    let ir_reqs = match iro.input_requests.clone() {
                        Some(r) => r,
                        None => {
                            let e: McpSdkError = RpcError::invalid_params()
                                .with_message("InputRequiredResult with no inputRequests")
                                .into();
                            return Err(e);
                        }
                    };
                    let input_responses = self.process_input_requests(ir_reqs).await?;
                    current_params = current_params.with_input_responses(input_responses);
                    if let Some(state) = iro.request_state.clone() {
                        current_params = current_params.with_request_state(state);
                    }
                }
                _ => {
                    let e: McpSdkError = RpcError::internal_error()
                        .with_message("Unexpected MRTR result")
                        .into();
                    return Err(e);
                }
            }
        }
        let e: McpSdkError = RpcError::internal_error()
            .with_message("MRTR: exceeded maximum rounds (10)")
            .into();
        Err(e)
    }
    /// MRTR-aware `prompts/get`. Retries up to 10 rounds.
    pub async fn get_prompt(&self, params: GetPromptRequestParams) -> SdkResult<GetPromptResult> {
        const MAX_ROUNDS: usize = 10;
        let span = tracing::info_span!("mcp.mrtr", method = "prompts/get");
        let _enter = span.enter();
        let mut current_params = params;
        for _round in 0..MAX_ROUNDS {
            let response = self
                .request(
                    crate::schema::schema_utils::RequestFromClient::GetPromptRequest(
                        current_params.clone(),
                    ),
                    Some(MRTR_ROUND_TIMEOUT),
                )
                .await?;
            match response {
                ServerResult::GetPromptResult(r) => {
                    if r.result_type != "complete" {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Invalid resultType for GetPromptResult")
                            .into();
                        return Err(e);
                    }
                    return Ok(r);
                }
                ServerResult::InputRequiredResult(iro) => {
                    if iro.result_type != "input_required" {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Invalid resultType for InputRequiredResult")
                            .into();
                        return Err(e);
                    }
                    let ir_reqs = match iro.input_requests.clone() {
                        Some(r) => r,
                        None => {
                            let e: McpSdkError = RpcError::invalid_params()
                                .with_message("InputRequiredResult with no inputRequests")
                                .into();
                            return Err(e);
                        }
                    };
                    let input_responses = self.process_input_requests(ir_reqs).await?;
                    current_params = current_params.with_input_responses(input_responses);
                    if let Some(state) = iro.request_state.clone() {
                        current_params = current_params.with_request_state(state);
                    }
                }
                _ => {
                    let e: McpSdkError = RpcError::internal_error()
                        .with_message("Unexpected MRTR result")
                        .into();
                    return Err(e);
                }
            }
        }
        let e: McpSdkError = RpcError::internal_error()
            .with_message("MRTR: exceeded maximum rounds (10)")
            .into();
        Err(e)
    }
    /// MRTR-aware `resources/read`. Retries up to 10 rounds.
    pub async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
    ) -> SdkResult<ReadResourceResult> {
        const MAX_ROUNDS: usize = 10;
        let span = tracing::info_span!("mcp.mrtr", method = "resources/read");
        let _enter = span.enter();
        let mut current_params = params;
        for _round in 0..MAX_ROUNDS {
            let response = self
                .request(
                    crate::schema::schema_utils::RequestFromClient::ReadResourceRequest(
                        current_params.clone(),
                    ),
                    Some(MRTR_ROUND_TIMEOUT),
                )
                .await?;
            match response {
                ServerResult::ReadResourceResult(r) => {
                    if r.result_type != "complete" {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Invalid resultType for ReadResourceResult")
                            .into();
                        return Err(e);
                    }
                    return Ok(r);
                }
                ServerResult::InputRequiredResult(iro) => {
                    if iro.result_type != "input_required" {
                        let e: McpSdkError = RpcError::internal_error()
                            .with_message("Invalid resultType for InputRequiredResult")
                            .into();
                        return Err(e);
                    }
                    let ir_reqs = match iro.input_requests.clone() {
                        Some(r) => r,
                        None => {
                            let e: McpSdkError = RpcError::invalid_params()
                                .with_message("InputRequiredResult with no inputRequests")
                                .into();
                            return Err(e);
                        }
                    };
                    let input_responses = self.process_input_requests(ir_reqs).await?;
                    current_params = current_params
                        .with_input_responses(input_responses)
                        .with_request_state(iro.request_state.unwrap_or_default());
                }
                _ => {
                    let e: McpSdkError = RpcError::internal_error()
                        .with_message("Unexpected MRTR result")
                        .into();
                    return Err(e);
                }
            }
        }
        let e: McpSdkError = RpcError::internal_error()
            .with_message("MRTR: exceeded maximum rounds (10)")
            .into();
        Err(e)
    }
}

impl ClientRuntime {
    #[cfg(feature = "streamable-http")]
    pub(crate) async fn start_stream(
        &self,
        messages: ClientMessages,
        timeout: Option<Duration>,
    ) -> SdkResult<Option<ServerMessages>> {
        use futures::stream::{AbortHandle, Abortable};
        use rust_mcp_schema::schema_utils::McpMessage;

        use crate::IdGenerator;
        let stream_id: StreamId = self.stream_id_gen.generate();

        let has_request = match &messages {
            ClientMessages::Single(client_message) => client_message.is_request(),
            ClientMessages::Batch(_) => unreachable!(),
        };

        let transport: Arc<
            dyn TransportDispatcher<
                ServerMessages,
                MessageFromClient,
                ServerMessage,
                ClientMessages,
                ClientMessage,
            >,
        > = Arc::new(self.new_transport().await?);

        let mut stream = transport.start().await?;

        let send_task = async {
            let result = transport.send_message(messages, timeout).await?;

            Ok::<_, McpSdkError>(result)
        };

        if !has_request {
            return send_task.await;
        }

        let (abort_recv_handle, abort_recv_reg) = AbortHandle::new_pair();

        let receive_task = async {
            loop {
                tokio::select! {
                    Some(mcp_messages) = stream.next() =>{

                        match mcp_messages {
                            ServerMessages::Single(server_message) => {
                                let result = self.handle_message(server_message, &transport).await?;
                                if let Some(result) = result {
                                    transport.send_message(ClientMessages::Single(result), None).await?;
                                }
                            }
                            ServerMessages::Batch(_) => {}
                        }
                        // close the stream after all messages are sent, unless it is a standalone stream
                        if !stream_id.eq(DEFAULT_STREAM_ID){
                            return  Ok::<_, McpSdkError>(());
                        }
                    }
                }
            }
        };

        let receive_task = Abortable::new(receive_task, abort_recv_reg);

        // Pin the tasks to ensure they are not moved
        tokio::pin!(send_task);
        tokio::pin!(receive_task);

        // Run both tasks with cancellation logic
        let (send_res, _) = tokio::select! {
            res = &mut send_task => {
                // cancel the receive_task task, to cover the case where send_task returns with error
                abort_recv_handle.abort();
                (res, receive_task.await) // Wait for receive_task to finish (it should exit due to cancellation)
            }
            res = &mut receive_task => {
                (send_task.await, res)
            }
        };
        send_res
    }
}

#[async_trait]
impl McpClient for ClientRuntime {
    fn client_details(&self) -> &ClientDetails {
        &self.client_details
    }

    /// `tools/list`, with SEP-2243 enforcement and SEP-2549 caching.
    async fn request_tool_list(
        &self,
        params: Option<crate::schema::PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListToolsResult> {
        let cur_params = params.unwrap_or_default();
        let cursor_key = cur_params.cursor.clone().unwrap_or_default();
        let params_val = serde_json::to_value(&cur_params).unwrap_or_default();

        if !response_cache::check_mrtr(&params_val) {
            let auth = self.auth_principal().await;
            let cached = {
                let mut lock = self.response_cache.lock().await;
                lock.as_mut().and_then(|c| {
                    c.get("tools/list", &cursor_key, auth.as_deref())
                        .map(|e| e.data.clone())
                })
            };
            if let Some(data) = cached {
                let result: crate::schema::ListToolsResult =
                    serde_json::from_value(data).map_err(|e| {
                        RpcError::internal_error()
                            .with_message(format!("Cache deserialization error: {e}"))
                    })?;
                return Ok(result);
            }
        }

        let response = self
            .request(
                crate::schema::schema_utils::RequestFromClient::ListToolsRequest(cur_params),
                None,
            )
            .await?;
        let mut result: crate::schema::ListToolsResult = response.try_into()?;
        crate::tool_param_headers::filter_and_register(
            &mut result.tools,
            &self.tool_header_registry,
        );

        if !response_cache::check_mrtr(&params_val) {
            if let Ok(val) = serde_json::to_value(&result) {
                let (ttl, private) = response_cache::extract_cache_attrs(&val);
                let auth = self.auth_principal().await;
                let mut lock = self.response_cache.lock().await;
                if let Some(cache) = lock.as_mut() {
                    cache.put("tools/list", &cursor_key, val, ttl, private, auth);
                }
            }
        }

        Ok(result)
    }

    /// `prompts/list` with SEP-2549 caching.
    async fn request_prompt_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListPromptsResult> {
        let cur_params = params.unwrap_or_default();
        let cursor_key = cur_params.cursor.clone().unwrap_or_default();
        let params_val = serde_json::to_value(&cur_params).unwrap_or_default();

        if !response_cache::check_mrtr(&params_val) {
            let auth = self.auth_principal().await;
            let cached = {
                let mut lock = self.response_cache.lock().await;
                lock.as_mut().and_then(|c| {
                    c.get("prompts/list", &cursor_key, auth.as_deref())
                        .map(|e| e.data.clone())
                })
            };
            if let Some(data) = cached {
                let result: crate::schema::ListPromptsResult = serde_json::from_value(data)
                    .map_err(|e| {
                        RpcError::internal_error()
                            .with_message(format!("Cache deserialization error: {e}"))
                    })?;
                return Ok(result);
            }
        }

        let response = self
            .request(RequestFromClient::ListPromptsRequest(cur_params), None)
            .await?;
        let result: crate::schema::ListPromptsResult = response.try_into()?;

        if !response_cache::check_mrtr(&params_val) {
            if let Ok(val) = serde_json::to_value(&result) {
                let (ttl, private) = response_cache::extract_cache_attrs(&val);
                let auth = self.auth_principal().await;
                let mut lock = self.response_cache.lock().await;
                if let Some(cache) = lock.as_mut() {
                    cache.put("prompts/list", &cursor_key, val, ttl, private, auth);
                }
            }
        }

        Ok(result)
    }

    /// `resources/list` with SEP-2549 caching.
    async fn request_resource_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourcesResult> {
        let cur_params = params.unwrap_or_default();
        let cursor_key = cur_params.cursor.clone().unwrap_or_default();
        let params_val = serde_json::to_value(&cur_params).unwrap_or_default();

        if !response_cache::check_mrtr(&params_val) {
            let auth = self.auth_principal().await;
            let cached = {
                let mut lock = self.response_cache.lock().await;
                lock.as_mut().and_then(|c| {
                    c.get("resources/list", &cursor_key, auth.as_deref())
                        .map(|e| e.data.clone())
                })
            };
            if let Some(data) = cached {
                let result: crate::schema::ListResourcesResult = serde_json::from_value(data)
                    .map_err(|e| {
                        RpcError::internal_error()
                            .with_message(format!("Cache deserialization error: {e}"))
                    })?;
                return Ok(result);
            }
        }

        let response = self
            .request(RequestFromClient::ListResourcesRequest(cur_params), None)
            .await?;
        let result: crate::schema::ListResourcesResult = response.try_into()?;

        if !response_cache::check_mrtr(&params_val) {
            if let Ok(val) = serde_json::to_value(&result) {
                let (ttl, private) = response_cache::extract_cache_attrs(&val);
                let auth = self.auth_principal().await;
                let mut lock = self.response_cache.lock().await;
                if let Some(cache) = lock.as_mut() {
                    cache.put("resources/list", &cursor_key, val, ttl, private, auth);
                }
            }
        }

        Ok(result)
    }

    /// `resources/templates/list` with SEP-2549 caching.
    async fn request_resource_template_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourceTemplatesResult> {
        let cur_params = params.unwrap_or_default();
        let cursor_key = cur_params.cursor.clone().unwrap_or_default();
        let params_val = serde_json::to_value(&cur_params).unwrap_or_default();

        if !response_cache::check_mrtr(&params_val) {
            let auth = self.auth_principal().await;
            let cached = {
                let mut lock = self.response_cache.lock().await;
                lock.as_mut().and_then(|c| {
                    c.get("resources/templates/list", &cursor_key, auth.as_deref())
                        .map(|e| e.data.clone())
                })
            };
            if let Some(data) = cached {
                let result: crate::schema::ListResourceTemplatesResult =
                    serde_json::from_value(data).map_err(|e| {
                        RpcError::internal_error()
                            .with_message(format!("Cache deserialization error: {e}"))
                    })?;
                return Ok(result);
            }
        }

        let response = self
            .request(
                RequestFromClient::ListResourceTemplatesRequest(cur_params),
                None,
            )
            .await?;
        let result: crate::schema::ListResourceTemplatesResult = response.try_into()?;

        if !response_cache::check_mrtr(&params_val) {
            if let Ok(val) = serde_json::to_value(&result) {
                let (ttl, private) = response_cache::extract_cache_attrs(&val);
                let auth = self.auth_principal().await;
                let mut lock = self.response_cache.lock().await;
                if let Some(cache) = lock.as_mut() {
                    cache.put(
                        "resources/templates/list",
                        &cursor_key,
                        val,
                        ttl,
                        private,
                        auth,
                    );
                }
            }
        }

        Ok(result)
    }

    /// `resources/read` with SEP-2549 caching.
    async fn request_resource_read(
        &self,
        params: ReadResourceRequestParams,
    ) -> SdkResult<crate::schema::ReadResourceResult> {
        let params_val = serde_json::to_value(&params).unwrap_or_default();

        if !response_cache::check_mrtr(&params_val) {
            let auth = self.auth_principal().await;
            let cached = {
                let mut lock = self.response_cache.lock().await;
                lock.as_mut().and_then(|c| {
                    c.get("resources/read", &params.uri, auth.as_deref())
                        .map(|e| e.data.clone())
                })
            };
            if let Some(data) = cached {
                let result: crate::schema::ReadResourceResult = serde_json::from_value(data)
                    .map_err(|e| {
                        RpcError::internal_error()
                            .with_message(format!("Cache deserialization error: {e}"))
                    })?;
                return Ok(result);
            }
        }

        let response = self
            .request(RequestFromClient::ReadResourceRequest(params.clone()), None)
            .await?;
        let result: crate::schema::ReadResourceResult = response.try_into()?;

        if !response_cache::check_mrtr(&params_val) {
            if let Ok(val) = serde_json::to_value(&result) {
                let (ttl, private) = response_cache::extract_cache_attrs(&val);
                let auth = self.auth_principal().await;
                let mut lock = self.response_cache.lock().await;
                if let Some(cache) = lock.as_mut() {
                    cache.put("resources/read", &params.uri, val, ttl, private, auth);
                }
            }
        }

        Ok(result)
    }

    /// `server/discover` with SEP-2549 caching.
    async fn request_discover(
        &self,
        params: crate::schema::RequestParams,
    ) -> SdkResult<crate::schema::DiscoverResult> {
        let params_val = serde_json::to_value(&params).unwrap_or_default();

        if !response_cache::check_mrtr(&params_val) {
            let auth = self.auth_principal().await;
            let cached = {
                let mut lock = self.response_cache.lock().await;
                lock.as_mut().and_then(|c| {
                    c.get("server/discover", "", auth.as_deref())
                        .map(|e| e.data.clone())
                })
            };
            if let Some(data) = cached {
                let result: crate::schema::DiscoverResult =
                    serde_json::from_value(data).map_err(|e| {
                        RpcError::internal_error()
                            .with_message(format!("Cache deserialization error: {e}"))
                    })?;
                return Ok(result);
            }
        }

        let response = self
            .request(RequestFromClient::DiscoverRequest(params.clone()), None)
            .await?;
        let result: crate::schema::DiscoverResult = response.try_into()?;

        if !response_cache::check_mrtr(&params_val) {
            if let Ok(val) = serde_json::to_value(&result) {
                let (ttl, private) = response_cache::extract_cache_attrs(&val);
                let auth = self.auth_principal().await;
                let mut lock = self.response_cache.lock().await;
                if let Some(cache) = lock.as_mut() {
                    cache.put("server/discover", "", val, ttl, private, auth);
                }
            }
        }

        Ok(result)
    }

    async fn send(
        &self,
        message: MessageFromClient,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ServerMessage>> {
        #[cfg(feature = "streamable-http")]
        {
            if self.transport_options.is_some() {
                let outgoing_request_id = self
                    .request_id_gen
                    .request_id_for_message(&message, request_id);
                let mcp_message = ClientMessage::from_message(message, outgoing_request_id)?;

                // telemetry
                if let Some(observer) = self.message_observer.as_ref() {
                    observer.on_send(&mcp_message);
                }

                let response = self
                    .start_stream(ClientMessages::Single(mcp_message), request_timeout)
                    .await?;
                return response
                    .map(|r| r.as_single())
                    .transpose()
                    .map_err(|err| err.into());
            }
        }

        let transport_map = self.transport_map.read().await;

        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        let outgoing_request_id = self
            .request_id_gen
            .request_id_for_message(&message, request_id);

        let mcp_message = ClientMessage::from_message(message, outgoing_request_id)?;

        // telemetry
        if let Some(observer) = self.message_observer.as_ref() {
            observer.on_send(&mcp_message);
        }

        let response = transport
            .send_message(ClientMessages::Single(mcp_message), request_timeout)
            .await?;
        response
            .map(|r| r.as_single())
            .transpose()
            .map_err(|err| err.into())
    }

    async fn session_id(&self) -> Option<SessionId> {
        None
    }
    async fn start(self: Arc<Self>) -> SdkResult<()> {
        #[cfg(feature = "streamable-http")]
        {
            if self.transport_options.is_some() {
                return Ok(());
            }
        }

        self.start_standalone().await
    }

    async fn is_shut_down(&self) -> bool {
        let result = self.is_shut_down.lock().await;
        *result
    }

    async fn shut_down(&self) -> SdkResult<()> {
        let mut is_shut_down_lock = self.is_shut_down.lock().await;
        *is_shut_down_lock = true;

        let mut transport_map = self.transport_map.write().await;
        let transport_option = transport_map.take();
        drop(transport_map);
        if let Some(transport) = transport_option {
            let _ = transport.shut_down().await;
        }

        let mut tasks_lock = self.handlers.lock().await;
        let join_handlers: Vec<_> = tasks_lock.drain(..).collect();
        drop(tasks_lock);

        for handle in join_handlers {
            handle.abort();
        }

        Ok(())
    }

    async fn terminate_session(&self) {
        let _ = self.shut_down().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};
    use tokio::io::ReadBuf;

    struct PendingReader {
        polls: Arc<AtomicUsize>,
    }

    impl AsyncRead for PendingReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            self.polls.fetch_add(1, Ordering::Relaxed);
            Poll::Pending
        }
    }

    #[tokio::test]
    async fn process_error_reader_waits_without_busy_polling() {
        let polls = Arc::new(AtomicUsize::new(0));
        let input = PendingReader {
            polls: Arc::clone(&polls),
        };
        let mut lines = BufReader::new(input).lines();

        let result =
            tokio::time::timeout(Duration::from_millis(25), next_process_error(&mut lines)).await;

        assert!(result.is_err(), "pending stderr should remain pending");
        assert!(
            polls.load(Ordering::Relaxed) <= 2,
            "idle stderr was polled repeatedly"
        );
    }
}
