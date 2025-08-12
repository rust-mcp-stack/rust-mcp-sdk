use crate::{
    hyper_servers::{
        app_state::AppState,
        error::{TransportServerError, TransportServerResult},
    },
    mcp_runtimes::server_runtime::DEFAULT_STREAM_ID,
    utils::remove_query_and_hash,
};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::{collections::HashMap, sync::Arc};

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route(
        remove_query_and_hash(&state.sse_message_endpoint).as_str(),
        post(handle_messages),
    )
}

pub async fn handle_messages(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    message: String,
) -> TransportServerResult<impl IntoResponse> {
    let session_id = params
        .get("sessionId")
        .ok_or(TransportServerError::SessionIdMissing)?;

    // transmit to the readable stream, that transport is reading from
    let transmit =
        state
            .session_store
            .get(session_id)
            .await
            .ok_or(TransportServerError::SessionIdInvalid(
                session_id.to_string(),
            ))?;

    let transmit = transmit.lock().await;

    transmit
        .consume_payload_string(DEFAULT_STREAM_ID, &message)
        .await
        .map_err(|err| {
            tracing::trace!("{}", err);
            TransportServerError::StreamIoError(err.to_string())
        })?;

    Ok(axum::http::StatusCode::ACCEPTED)
}
