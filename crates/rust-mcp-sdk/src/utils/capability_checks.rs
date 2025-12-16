use rust_mcp_schema::{
    CallToolRequest, CancelTaskRequest, CompleteRequest, GetPromptRequest, GetTaskPayloadRequest,
    GetTaskRequest, ListPromptsRequest, ListResourceTemplatesRequest, ListResourcesRequest,
    ListTasksRequest, ListToolsRequest, LoggingMessageNotification, PromptListChangedNotification,
    ReadResourceRequest, ResourceUpdatedNotification, RpcError, ServerCapabilities,
    SetLevelRequest, SubscribeRequest, ToolListChangedNotification, UnsubscribeRequest,
};

/// Asserts that server capabilities support the requested method.
///
/// Verifies that the server has the necessary capabilities to handle the given request method.
/// If the server is not initialized or lacks a required capability, an error is returned.
/// This can be utilized to avoid sending requests when the opposing party lacks support for them.
pub fn assert_server_request_capabilities(
    capabilities: &ServerCapabilities,
    request_method: &str,
) -> std::result::Result<(), RpcError> {
    let entity = "Server";

    // logging support
    if [SetLevelRequest::method_value()].contains(&request_method) && capabilities.logging.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "logging",
                request_method,
            )),
        );
    }

    // propmpts support
    if [
        GetPromptRequest::method_value(),
        ListPromptsRequest::method_value(),
    ]
    .contains(&request_method)
        && capabilities.prompts.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "prompts",
                request_method,
            )),
        );
    }

    // resources support
    if [
        ListResourcesRequest::method_value(),
        ListResourceTemplatesRequest::method_value(),
        ReadResourceRequest::method_value(),
        SubscribeRequest::method_value(),
        UnsubscribeRequest::method_value(),
    ]
    .contains(&request_method)
        && capabilities.resources.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "resources",
                request_method,
            )),
        );
    }

    // call tool support
    if [
        CallToolRequest::method_value(),
        ListToolsRequest::method_value(),
    ]
    .contains(&request_method)
        && capabilities.tools.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "tools",
                request_method,
            )),
        );
    }

    // completion
    if request_method.eq(CompleteRequest::method_value()) {
        if capabilities.completions.is_none() {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "completions",
                    request_method,
                )),
            );
        }
    }

    if [
        GetTaskRequest::method_value(),
        GetTaskPayloadRequest::method_value(),
        CancelTaskRequest::method_value(),
        ListTasksRequest::method_value(),
    ]
    .contains(&request_method)
        && capabilities.tasks.is_none()
    {
        if let Some(tasks) = capabilities.tasks.as_ref() {
            if request_method.eq(ListTasksRequest::method_value()) && !tasks.can_list_tasks() {
                return Err(
                    RpcError::internal_error().with_message(format_assertion_message(
                        entity,
                        "listing tasks",
                        request_method,
                    )),
                );
            }
            if request_method.eq(CancelTaskRequest::method_value()) && !tasks.can_list_tasks() {
                return Err(
                    RpcError::internal_error().with_message(format_assertion_message(
                        entity,
                        "task cancellation",
                        request_method,
                    )),
                );
            }
        } else {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "tools",
                    request_method,
                )),
            );
        }
    }

    Ok(())
}

/// Asserts that the server supports the requested notification.
///
/// Verifies that the server advertises support for the notification type,
/// allowing callers to avoid sending notifications that the server does not
/// support. This can be used to prevent issuing requests to peers that lack
/// the required capability.
pub fn assert_server_notification_capabilities(
    capabilities: &ServerCapabilities,
    notification_method: &String,
) -> std::result::Result<(), RpcError> {
    let entity = "Server";

    if *notification_method == LoggingMessageNotification::method_value()
        && capabilities.logging.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "logging",
                notification_method,
            )),
        );
    }
    if *notification_method == ResourceUpdatedNotification::method_value()
        && capabilities.resources.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "notifying about resources",
                notification_method,
            )),
        );
    }
    if *notification_method == ToolListChangedNotification::method_value()
        && capabilities.tools.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "notifying of tool list changes",
                notification_method,
            )),
        );
    }
    if *notification_method == PromptListChangedNotification::method_value()
        && capabilities.prompts.is_none()
    {
        return Err(
            RpcError::internal_error().with_message(format_assertion_message(
                entity,
                "notifying of prompt list changes",
                notification_method,
            )),
        );
    }

    Ok(())
}

/// Formats an assertion error message for unsupported capabilities.
///
/// Constructs a string describing that a specific entity (e.g., server or client) lacks
/// support for a required capability, needed for a particular method.
///
/// # Arguments
/// - `entity`: The name of the entity (e.g., "Server" or "Client") that lacks support.
/// - `capability`: The name of the unsupported capability or tool.
/// - `method_name`: The name of the method requiring the capability.
///
/// # Returns
/// A formatted string detailing the unsupported capability error.
///
/// # Examples
/// ```ignore
/// let msg = format_assertion_message("Server", "tools", rust_mcp_schema::ListResourcesRequest::method_value());
/// assert_eq!(msg, "Server does not support resources (required for resources/list)");
/// ```
pub fn format_assertion_message(entity: &str, capability: &str, method_name: &str) -> String {
    format!("{entity} does not support {capability} (required for {method_name})")
}
